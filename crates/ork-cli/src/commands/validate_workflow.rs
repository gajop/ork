use anyhow::{Context, Result, bail};
use clap::Args;
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Args)]
pub struct ValidateWorkflow {
    /// Path to workflow YAML file
    pub file: String,
}

impl ValidateWorkflow {
    pub async fn execute(self) -> Result<()> {
        let data = std::fs::read_to_string(&self.file)
            .with_context(|| format!("Failed to read workflow file: {}", self.file))?;

        let workflow = validate_workflow_yaml_str(&data)
            .with_context(|| format!("workflow validation failed: {}", self.file))?;

        println!(
            "✓ Workflow '{}' is valid (schema, {} task(s))",
            workflow.name,
            workflow.tasks.len()
        );
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct WorkflowSchema {
    name: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    types: BTreeMap<String, TypeExpr>,
    tasks: BTreeMap<String, TaskDefinition>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct TaskDefinition {
    executor: ExecutorKind,
    file: Option<String>,
    command: Option<String>,
    job: Option<String>,
    module: Option<String>,
    function: Option<String>,
    timeout: Option<u64>,
    retries: Option<u32>,
    #[serde(default)]
    depends_on: Vec<String>,
    input_type: TypeExpr,
    output_type: TypeExpr,
    inputs: BTreeMap<String, Binding>,
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum ExecutorKind {
    #[serde(alias = "shell")]
    Process,
    #[serde(alias = "cloud_run")]
    CloudRun,
    Python,
    Library,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum Binding {
    Const(ConstBinding),
    Ref(RefBinding),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ConstBinding {
    #[serde(rename = "const")]
    value: Value,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct RefBinding {
    #[serde(rename = "ref")]
    path: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(untagged)]
enum TypeExpr {
    Name(String),
    Array(Vec<TypeExpr>),
    Object(BTreeMap<String, TypeExpr>),
}

fn validate_workflow_yaml_str(data: &str) -> Result<WorkflowSchema> {
    let workflow: WorkflowSchema =
        serde_yaml::from_str(data).context("Failed to parse workflow YAML")?;
    workflow.validate()?;
    Ok(workflow)
}

impl WorkflowSchema {
    fn validate(&self) -> Result<()> {
        if self.name.trim().is_empty() {
            bail!("workflow.name must not be empty");
        }
        if let Some(description) = &self.description
            && description.trim().is_empty()
        {
            bail!("workflow.description must not be empty when set");
        }
        if self.tasks.is_empty() {
            bail!("workflow.tasks must contain at least one task");
        }

        for alias in self.types.keys() {
            validate_identifier(alias, &format!("types.{}", alias))?;
            if is_builtin(alias) {
                bail!(
                    "types.{} cannot use builtin type name '{}'; choose a distinct alias",
                    alias,
                    alias
                );
            }
        }

        for (alias, expr) in &self.types {
            let mut stack = Vec::new();
            self.validate_type_expr(expr, &format!("types.{}", alias), &mut stack)?;
        }

        self.validate_task_graph()?;

        for (task_name, task_def) in &self.tasks {
            validate_identifier(task_name, &format!("tasks.{}", task_name))?;
            let _ = (task_def.timeout, task_def.retries);

            self.validate_executor_requirements(task_name, task_def)?;

            let mut stack = Vec::new();
            self.validate_type_expr(
                &task_def.input_type,
                &format!("tasks.{}.input_type", task_name),
                &mut stack,
            )?;

            let mut stack = Vec::new();
            self.validate_type_expr(
                &task_def.output_type,
                &format!("tasks.{}.output_type", task_name),
                &mut stack,
            )?;

            let input_fields = self.resolve_input_fields(
                &task_def.input_type,
                &format!("tasks.{}.input_type", task_name),
            )?;

            let expected_keys: HashSet<&str> = input_fields.keys().map(String::as_str).collect();
            let actual_keys: HashSet<&str> = task_def.inputs.keys().map(String::as_str).collect();

            let missing: Vec<&str> = expected_keys.difference(&actual_keys).copied().collect();
            if !missing.is_empty() {
                bail!(
                    "tasks.{}.inputs missing binding(s) for: {}",
                    task_name,
                    missing.join(", ")
                );
            }

            let extra: Vec<&str> = actual_keys.difference(&expected_keys).copied().collect();
            if !extra.is_empty() {
                bail!(
                    "tasks.{}.inputs has extra binding(s) not present in input_type: {}",
                    task_name,
                    extra.join(", ")
                );
            }

            for (arg_name, binding) in &task_def.inputs {
                let expected = input_fields.get(arg_name).expect("input key should exist");
                let binding_path = format!("tasks.{}.inputs.{}", task_name, arg_name);
                match binding {
                    Binding::Const(c) => {
                        self.validate_const_against_type(&c.value, expected, &binding_path)?;
                    }
                    Binding::Ref(r) => {
                        if r.path.trim().is_empty() {
                            bail!("{}.ref must not be empty", binding_path);
                        }
                        let ref_type = self.resolve_ref_type(&r.path, task_name)?;
                        let expected_norm = self.normalize_type(expected)?;
                        let actual_norm = self.normalize_type(&ref_type)?;
                        if !is_assignable(&expected_norm, &actual_norm) {
                            bail!(
                                "{} type mismatch: expected {:?}, got {:?} from ref {}",
                                binding_path,
                                expected_norm,
                                actual_norm,
                                r.path
                            );
                        }
                    }
                }
            }
        }

        Ok(())
    }

    fn validate_executor_requirements(&self, task_name: &str, task: &TaskDefinition) -> Result<()> {
        match task.executor {
            ExecutorKind::Python => {
                if let Some(function) = task.function.as_deref()
                    && function.trim().is_empty()
                {
                    bail!(
                        "tasks.{} has empty 'function'; omit it or provide a non-empty name",
                        task_name
                    );
                }
                let has_module = task
                    .module
                    .as_deref()
                    .map(|m| !m.trim().is_empty())
                    .unwrap_or(false);
                let has_file = task
                    .file
                    .as_deref()
                    .map(|f| !f.trim().is_empty())
                    .unwrap_or(false);
                if !has_module && !has_file {
                    bail!(
                        "tasks.{} with executor=python requires non-empty 'module' or 'file'",
                        task_name
                    );
                }
            }
            ExecutorKind::Process => {
                let has_command = task
                    .command
                    .as_deref()
                    .map(|c| !c.trim().is_empty())
                    .unwrap_or(false);
                let has_file = task
                    .file
                    .as_deref()
                    .map(|f| !f.trim().is_empty())
                    .unwrap_or(false);
                if !has_command && !has_file {
                    bail!(
                        "tasks.{} with executor=process requires non-empty 'command' or 'file'",
                        task_name
                    );
                }
            }
            ExecutorKind::CloudRun => {
                let has_job = task
                    .job
                    .as_deref()
                    .map(|j| !j.trim().is_empty())
                    .unwrap_or(false);
                if !has_job {
                    bail!(
                        "tasks.{} with executor=cloudrun requires non-empty 'job'",
                        task_name
                    );
                }
            }
            ExecutorKind::Library => {
                let has_file = task
                    .file
                    .as_deref()
                    .map(|f| !f.trim().is_empty())
                    .unwrap_or(false);
                if !has_file {
                    bail!(
                        "tasks.{} with executor=library requires non-empty 'file'",
                        task_name
                    );
                }
            }
        }
        Ok(())
    }

    fn validate_task_graph(&self) -> Result<()> {
        for (task_name, task) in &self.tasks {
            let mut seen = HashSet::new();
            for dep in &task.depends_on {
                if dep == task_name {
                    bail!("tasks.{} cannot depend on itself", task_name);
                }
                if !self.tasks.contains_key(dep) {
                    bail!(
                        "tasks.{} depends_on references unknown task '{}'",
                        task_name,
                        dep
                    );
                }
                if !seen.insert(dep) {
                    bail!("tasks.{} depends_on has duplicate '{}'", task_name, dep);
                }
            }
        }

        let mut states: HashMap<&str, VisitState> = HashMap::new();
        for task_name in self.tasks.keys().map(String::as_str) {
            self.dfs_cycle(task_name, &mut states)?;
        }

        Ok(())
    }

    fn dfs_cycle<'a>(
        &'a self,
        task_name: &'a str,
        states: &mut HashMap<&'a str, VisitState>,
    ) -> Result<()> {
        if let Some(state) = states.get(task_name) {
            return match state {
                VisitState::Visiting => bail!(
                    "workflow has a dependency cycle involving task '{}'",
                    task_name
                ),
                VisitState::Visited => Ok(()),
            };
        }

        states.insert(task_name, VisitState::Visiting);

        let task = self
            .tasks
            .get(task_name)
            .with_context(|| format!("missing task '{}' during cycle check", task_name))?;

        for dep in &task.depends_on {
            self.dfs_cycle(dep, states)?;
        }

        states.insert(task_name, VisitState::Visited);
        Ok(())
    }

    fn validate_type_expr(
        &self,
        expr: &TypeExpr,
        path: &str,
        alias_stack: &mut Vec<String>,
    ) -> Result<()> {
        match expr {
            TypeExpr::Name(name) => {
                if is_builtin(name) {
                    return Ok(());
                }
                if let Some(alias) = name.strip_prefix("types.") {
                    if alias.is_empty() {
                        bail!(
                            "{} has invalid type reference '{}': missing alias name",
                            path,
                            name
                        );
                    }
                    let aliased = self.types.get(alias).with_context(|| {
                        format!("{} references unknown type alias '{}'", path, name)
                    })?;

                    if alias_stack.iter().any(|existing| existing == alias) {
                        alias_stack.push(alias.to_string());
                        bail!(
                            "{} has recursive type alias chain: {}",
                            path,
                            alias_stack.join(" -> ")
                        );
                    }

                    alias_stack.push(alias.to_string());
                    self.validate_type_expr(
                        aliased,
                        &format!("{} -> {}", path, name),
                        alias_stack,
                    )?;
                    alias_stack.pop();
                    return Ok(());
                }

                if is_legacy_builtin(name) {
                    bail!(
                        "{} uses legacy type '{}'; use builtins: {}",
                        path,
                        name,
                        BUILTIN_TYPES.join(", ")
                    );
                }

                bail!(
                    "{} has invalid type '{}'; expected builtin ({}) or alias (types.<Name>)",
                    path,
                    name,
                    BUILTIN_TYPES.join(", ")
                );
            }
            TypeExpr::Array(items) => {
                if items.len() != 1 {
                    bail!(
                        "{} array type must have exactly one element type, got {}",
                        path,
                        items.len()
                    );
                }
                self.validate_type_expr(&items[0], &format!("{}[]", path), alias_stack)
            }
            TypeExpr::Object(map) => {
                for (field, field_type) in map {
                    validate_identifier(field, &format!("{}.{}", path, field))?;
                    self.validate_type_expr(
                        field_type,
                        &format!("{}.{}", path, field),
                        alias_stack,
                    )?;
                }
                Ok(())
            }
        }
    }

    fn normalize_type(&self, expr: &TypeExpr) -> Result<TypeExpr> {
        self.normalize_type_with_stack(expr, &mut Vec::new())
    }

    fn normalize_type_with_stack(
        &self,
        expr: &TypeExpr,
        alias_stack: &mut Vec<String>,
    ) -> Result<TypeExpr> {
        match expr {
            TypeExpr::Name(name) if is_builtin(name) => Ok(TypeExpr::Name(name.clone())),
            TypeExpr::Name(name) => {
                let Some(alias) = name.strip_prefix("types.") else {
                    bail!(
                        "cannot normalize non-builtin non-alias type '{}'; expected types.<Name>",
                        name
                    );
                };
                if alias_stack.iter().any(|existing| existing == alias) {
                    alias_stack.push(alias.to_string());
                    bail!(
                        "recursive alias while normalizing type: {}",
                        alias_stack.join(" -> ")
                    );
                }
                let aliased = self
                    .types
                    .get(alias)
                    .with_context(|| format!("unknown type alias '{}'", name))?;
                alias_stack.push(alias.to_string());
                let normalized = self.normalize_type_with_stack(aliased, alias_stack)?;
                alias_stack.pop();
                Ok(normalized)
            }
            TypeExpr::Array(items) => {
                if items.len() != 1 {
                    bail!("array type must have exactly one element type");
                }
                Ok(TypeExpr::Array(vec![
                    self.normalize_type_with_stack(&items[0], alias_stack)?,
                ]))
            }
            TypeExpr::Object(map) => {
                let mut normalized = BTreeMap::new();
                for (k, v) in map {
                    normalized.insert(k.clone(), self.normalize_type_with_stack(v, alias_stack)?);
                }
                Ok(TypeExpr::Object(normalized))
            }
        }
    }

    fn resolve_input_fields(
        &self,
        expr: &TypeExpr,
        path: &str,
    ) -> Result<BTreeMap<String, TypeExpr>> {
        match self.normalize_type(expr)? {
            TypeExpr::Object(map) => Ok(map),
            other => bail!(
                "{} must resolve to an object type (named task arguments), got {:?}",
                path,
                other
            ),
        }
    }

    fn resolve_ref_type(&self, reference: &str, current_task: &str) -> Result<TypeExpr> {
        let parts: Vec<&str> = reference.split('.').collect();
        if parts.is_empty() || parts[0].is_empty() {
            bail!("invalid ref '{}': must not be empty", reference);
        }

        match parts[0] {
            "tasks" => self.resolve_task_ref(reference, &parts, current_task),
            _ => bail!(
                "invalid ref '{}': must be tasks.<task>.output[.<field>...]",
                reference
            ),
        }
    }

    fn resolve_task_ref(
        &self,
        reference: &str,
        parts: &[&str],
        current_task: &str,
    ) -> Result<TypeExpr> {
        if parts.len() < 3 || parts[1].is_empty() || parts[2] != "output" {
            bail!(
                "invalid ref '{}': task reference must be tasks.<task>.output[.<field>...]",
                reference
            );
        }

        let task_name = parts[1];
        let task = self.tasks.get(task_name).with_context(|| {
            format!("ref '{}' points to unknown task '{}'", reference, task_name)
        })?;

        if task_name == current_task {
            bail!(
                "ref '{}' is invalid: task '{}' cannot reference its own output",
                reference,
                current_task
            );
        }

        let current = self
            .tasks
            .get(current_task)
            .with_context(|| format!("current task '{}' is missing", current_task))?;
        if !current.depends_on.iter().any(|dep| dep == task_name) {
            bail!(
                "ref '{}' is invalid: task '{}' does not depend_on '{}'",
                reference,
                current_task,
                task_name
            );
        }

        self.resolve_field_path(&task.output_type, &parts[3..], reference)
    }

    fn resolve_field_path(
        &self,
        base: &TypeExpr,
        segments: &[&str],
        reference: &str,
    ) -> Result<TypeExpr> {
        let mut current = self.normalize_type(base)?;

        for segment in segments {
            let TypeExpr::Object(fields) = current else {
                bail!(
                    "ref '{}' cannot access field '{}' on non-object type",
                    reference,
                    segment
                );
            };

            let next = fields.get(*segment).with_context(|| {
                format!("ref '{}' points to missing field '{}'", reference, segment)
            })?;

            current = self.normalize_type(next)?;
        }

        Ok(current)
    }

    fn validate_const_against_type(
        &self,
        value: &Value,
        expected: &TypeExpr,
        path: &str,
    ) -> Result<()> {
        let normalized = self.normalize_type(expected)?;
        validate_const_against_normalized_type(value, &normalized, path)
    }
}

#[derive(Debug, Clone, Copy)]
enum VisitState {
    Visiting,
    Visited,
}

const BUILTIN_TYPES: &[&str] = &[
    "str",
    "int",
    "float",
    "bool",
    "date",
    "datetime_notz",
    "datetime_tz",
    "datetime_utc",
];

const LEGACY_BUILTIN_TYPES: &[&str] = &[
    "string",
    "integer",
    "number",
    "boolean",
    "datetime",
    "timestamp",
    "timestamp_s",
    "timestamp_ms",
    "time",
    "duration",
];

fn is_builtin(name: &str) -> bool {
    BUILTIN_TYPES.contains(&name)
}

fn is_legacy_builtin(name: &str) -> bool {
    LEGACY_BUILTIN_TYPES.contains(&name)
}

fn validate_identifier(name: &str, path: &str) -> Result<()> {
    if name.is_empty() {
        bail!("{} must not be empty", path);
    }

    let mut chars = name.chars();
    let Some(first) = chars.next() else {
        bail!("{} must not be empty", path);
    };

    if !(first.is_ascii_alphabetic() || first == '_') {
        bail!(
            "{} must start with [A-Za-z_] but starts with '{}'",
            path,
            first
        );
    }

    if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
        bail!("{} must contain only [A-Za-z0-9_]", path);
    }

    Ok(())
}

fn validate_const_against_normalized_type(
    value: &Value,
    expected: &TypeExpr,
    path: &str,
) -> Result<()> {
    match expected {
        TypeExpr::Name(name) => match name.as_str() {
            "str" => {
                if !value.is_string() {
                    bail!("{} expected str, got {}", path, value_kind(value));
                }
            }
            "int" => {
                if !(value.as_i64().is_some() || value.as_u64().is_some()) {
                    bail!("{} expected int, got {}", path, value_kind(value));
                }
            }
            "float" => {
                if !value.is_number() {
                    bail!("{} expected float, got {}", path, value_kind(value));
                }
            }
            "bool" => {
                if !value.is_boolean() {
                    bail!("{} expected bool, got {}", path, value_kind(value));
                }
            }
            "date" => {
                let s = value.as_str().with_context(|| {
                    format!("{} expected date string, got {}", path, value_kind(value))
                })?;
                chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d").with_context(|| {
                    format!("{} invalid date '{}', expected YYYY-MM-DD", path, s)
                })?;
            }
            "datetime_notz" => {
                let s = value.as_str().with_context(|| {
                    format!(
                        "{} expected datetime_notz string, got {}",
                        path,
                        value_kind(value)
                    )
                })?;
                let suffix = s.get(10..).unwrap_or("");
                if s.ends_with('Z') || suffix.contains('+') || suffix.contains('-') {
                    bail!(
                        "{} invalid datetime_notz '{}': timezone/offset is not allowed",
                        path,
                        s
                    );
                }
                chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f").with_context(|| {
                    format!(
                        "{} invalid datetime_notz '{}', expected YYYY-MM-DDTHH:MM:SS[.fraction]",
                        path,
                        s
                    )
                })?;
            }
            "datetime_tz" => {
                let s = value.as_str().with_context(|| {
                    format!(
                        "{} expected datetime_tz string, got {}",
                        path,
                        value_kind(value)
                    )
                })?;
                chrono::DateTime::parse_from_rfc3339(s).with_context(|| {
                    format!(
                        "{} invalid datetime_tz '{}', expected RFC3339 with timezone offset",
                        path, s
                    )
                })?;
            }
            "datetime_utc" => {
                let s = value.as_str().with_context(|| {
                    format!(
                        "{} expected datetime_utc string, got {}",
                        path,
                        value_kind(value)
                    )
                })?;
                chrono::DateTime::parse_from_rfc3339(s).with_context(|| {
                    format!(
                        "{} invalid datetime_utc '{}', expected RFC3339 UTC (Z suffix)",
                        path, s
                    )
                })?;
                if !s.ends_with('Z') {
                    bail!(
                        "{} invalid datetime_utc '{}': must use Z suffix (UTC)",
                        path,
                        s
                    );
                }
            }
            other => bail!("{} unsupported builtin type '{}'", path, other),
        },
        TypeExpr::Array(items) => {
            let element_type = items.first().with_context(|| {
                format!("{} invalid type: array must define one element type", path)
            })?;
            let arr = value
                .as_array()
                .with_context(|| format!("{} expected array, got {}", path, value_kind(value)))?;
            for (idx, item) in arr.iter().enumerate() {
                validate_const_against_normalized_type(
                    item,
                    element_type,
                    &format!("{}[{}]", path, idx),
                )?;
            }
        }
        TypeExpr::Object(fields) => {
            let obj = value
                .as_object()
                .with_context(|| format!("{} expected object, got {}", path, value_kind(value)))?;

            for key in fields.keys() {
                if !obj.contains_key(key) {
                    bail!("{} missing required field '{}'", path, key);
                }
            }
            for key in obj.keys() {
                if !fields.contains_key(key) {
                    bail!("{} has unexpected field '{}'", path, key);
                }
            }

            for (field, field_type) in fields {
                let field_value = obj
                    .get(field)
                    .with_context(|| format!("{} missing field '{}'", path, field))?;
                validate_const_against_normalized_type(
                    field_value,
                    field_type,
                    &format!("{}.{}", path, field),
                )?;
            }
        }
    }

    Ok(())
}

fn is_assignable(expected: &TypeExpr, actual: &TypeExpr) -> bool {
    if expected == actual {
        return true;
    }

    matches!(
        (expected, actual),
        (TypeExpr::Name(expected_name), TypeExpr::Name(actual_name))
            if expected_name == "datetime_tz" && actual_name == "datetime_utc"
    )
}

fn value_kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "bool",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    fn write_temp_workflow(content: &str) -> String {
        let path =
            std::env::temp_dir().join(format!("ork-validate-schema-{}.yaml", Uuid::new_v4()));
        std::fs::write(&path, content).expect("write temp workflow");
        path.to_string_lossy().to_string()
    }

    #[tokio::test]
    async fn test_validate_workflow_command_accepts_valid_schema_workflow() {
        let path = write_temp_workflow(
            r#"
name: wf_schema_valid
types:
  TaskResult:
    task: str
    status: str
    timestamp: datetime_utc

tasks:
  task_a:
    executor: python
    module: fast
    function: task_a
    input_type: {}
    output_type: types.TaskResult
    inputs: {}

  task_b:
    executor: python
    module: fast
    function: task_b
    depends_on: [task_a]
    input_type:
      a: types.TaskResult
    output_type: types.TaskResult
    inputs:
      a:
        ref: tasks.task_a.output
"#,
        );

        let result = ValidateWorkflow { file: path.clone() }.execute().await;
        let _ = std::fs::remove_file(path);
        assert!(result.is_ok(), "expected valid workflow schema: {result:?}");
    }

    #[test]
    fn test_validate_workflow_rejects_legacy_upstream_and_types() {
        let err = validate_workflow_yaml_str(
            r#"
name: fast_demo

tasks:
  task_a:
    executor: python
    module: fast
    function: task_a
    input_type: {}
    output_type:
      task: string
      status: string
      timestamp: number
    inputs: {}

  task_b:
    executor: python
    module: fast
    function: task_b
    depends_on: [task_a]
    input_type:
      upstream:
        task_a:
          task: string
          status: string
          timestamp: number
    output_type:
      task: string
      status: string
      timestamp: number
    inputs:
      upstream:
        ref: tasks.task_a.output
"#,
        )
        .expect_err("legacy schema should fail under strict validation");

        assert!(
            err.to_string().contains("legacy type 'string'")
                || err.to_string().contains("legacy type 'number'"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_validate_workflow_rejects_non_object_input_type() {
        let err = validate_workflow_yaml_str(
            r#"
name: wf_schema_bad_input_type

tasks:
  task_a:
    executor: process
    command: echo hi
    input_type: str
    output_type: str
    inputs:
      value:
        const: hi
"#,
        )
        .expect_err("non-object input_type should fail");

        assert!(
            err.to_string().contains("must resolve to an object type"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_validate_workflow_rejects_ref_to_non_dependency() {
        let err = validate_workflow_yaml_str(
            r#"
name: wf_schema_bad_ref

types:
  Out:
    v: int

tasks:
  a:
    executor: process
    command: echo a
    input_type: {}
    output_type: types.Out
    inputs: {}

  b:
    executor: process
    command: echo b
    input_type:
      x: types.Out
    output_type: types.Out
    inputs:
      x:
        ref: tasks.a.output
"#,
        )
        .expect_err("ref to non-dependency should fail");

        assert!(
            err.to_string().contains("does not depend_on"),
            "unexpected error: {err}"
        );
    }
}

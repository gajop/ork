use super::*;

impl<D: Database + 'static, E: ExecutorManager + 'static> Scheduler<D, E> {
    pub(super) async fn process_pending_runs(&self) -> Result<usize> {
        let runs = self.db.get_pending_runs().await?;

        if runs.is_empty() {
            return Ok(0);
        }

        let count = runs.len();

        let workflow_ids: Vec<Uuid> = runs
            .iter()
            .map(|r| r.workflow_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();

        let workflows = self.db.get_workflows_by_ids(&workflow_ids).await?;
        let workflow_map: HashMap<Uuid, Workflow> =
            workflows.into_iter().map(|w| (w.id, w)).collect();

        let mut workflow_tasks_map: HashMap<Uuid, Vec<WorkflowTask>> = HashMap::new();
        for workflow_id in workflow_map.keys() {
            let tasks = self.db.list_workflow_tasks(*workflow_id).await?;
            if !tasks.is_empty() {
                workflow_tasks_map.insert(*workflow_id, tasks);
            }
        }

        for run in &runs {
            if let Some(workflow) = workflow_map.get(&run.workflow_id) {
                let create_result =
                    if let Some(workflow_tasks) = workflow_tasks_map.get(&run.workflow_id) {
                        let tasks = build_run_tasks(run.id, workflow, workflow_tasks);
                        self.db.batch_create_dag_tasks(run.id, &tasks).await
                    } else if workflow.executor_type == "dag" {
                        Err(anyhow::anyhow!(
                            "workflow '{}' is missing compiled workflow_tasks",
                            workflow.name
                        ))
                    } else {
                        let task_count = workflow
                            .task_params
                            .as_ref()
                            .and_then(|p| json_inner(p).get("task_count"))
                            .and_then(|v: &serde_json::Value| v.as_i64())
                            .unwrap_or(3) as i32;
                        self.db
                            .batch_create_tasks(
                                run.id,
                                task_count,
                                &workflow.name,
                                &workflow.executor_type,
                            )
                            .await
                    };

                if let Err(e) = create_result {
                    error!("Failed to create tasks for run {}: {}", run.id, e);
                    let _ = self
                        .db
                        .update_run_status(run.id, "failed", Some(&e.to_string()))
                        .await;
                    continue;
                }

                if let Err(e) = self.db.update_run_status(run.id, "running", None).await {
                    error!("Failed to update run {} to running: {}", run.id, e);
                }
            }
        }

        Ok(count)
    }

    pub(super) async fn process_pending_tasks(&self) -> Result<usize> {
        let tasks = self
            .db
            .get_pending_tasks_with_workflow(self.config.max_tasks_per_batch)
            .await?;

        if tasks.is_empty() {
            return Ok(0);
        }

        let count = tasks.len();

        let workflow_ids: Vec<Uuid> = tasks
            .iter()
            .map(|task| task.workflow_id)
            .collect::<HashSet<_>>()
            .into_iter()
            .collect();
        let workflows = self.db.get_workflows_by_ids(&workflow_ids).await?;
        let workflow_map: HashMap<Uuid, Workflow> =
            workflows.into_iter().map(|w| (w.id, w)).collect();
        let workflow_map = Arc::new(workflow_map);

        let mut deps_by_run: HashMap<Uuid, HashSet<String>> = HashMap::new();
        for task in &tasks {
            if task.depends_on.is_empty() {
                continue;
            }
            let deps = deps_by_run.entry(task.run_id).or_default();
            for name in &task.depends_on {
                deps.insert(name.clone());
            }
        }

        let mut outputs_by_run: HashMap<Uuid, HashMap<String, serde_json::Value>> = HashMap::new();
        for (run_id, deps) in deps_by_run {
            let names: Vec<String> = deps.into_iter().collect();
            match self.db.get_task_outputs(run_id, &names).await {
                Ok(map) => {
                    outputs_by_run.insert(run_id, map);
                }
                Err(err) => {
                    error!(
                        "Failed to load upstream outputs for run {}: {}",
                        run_id, err
                    );
                }
            }
        }
        let outputs_by_run = Arc::new(outputs_by_run);

        let results = stream::iter(tasks)
            .map(|task_with_workflow| {
                let executor_manager = Arc::clone(&self.executor_manager);
                let status_tx = self.status_tx.clone();
                let workflow_map = workflow_map.clone();
                let outputs_by_run = outputs_by_run.clone();

                async move {
                    execute_task(
                        task_with_workflow,
                        workflow_map,
                        outputs_by_run,
                        &*executor_manager,
                        status_tx,
                    )
                    .await
                }
            })
            .buffer_unordered(self.config.max_concurrent_dispatches)
            .collect::<Vec<_>>()
            .await;

        let mut dispatch_updates: Vec<(Uuid, String)> = Vec::new();
        let mut failure_updates: Vec<StatusUpdate> = Vec::new();

        for (task_id, result) in results {
            match result {
                Ok(execution_name) => {
                    dispatch_updates.push((task_id, execution_name));
                }
                Err(e) => {
                    error!("Failed to dispatch task {}: {}", task_id, e);
                    failure_updates.push(StatusUpdate {
                        task_id,
                        status: "failed".to_string(),
                        log: None,
                        output: None,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        let db_update_start = Instant::now();
        if !dispatch_updates.is_empty() {
            let updates_ref: Vec<(Uuid, &str, Option<&str>, Option<&str>)> = dispatch_updates
                .iter()
                .map(|(id, execution)| (*id, "dispatched", Some(execution.as_str()), None))
                .collect();
            self.db.batch_update_task_status(&updates_ref).await?;
        }
        let db_update_ms = db_update_start.elapsed().as_millis();
        info!(
            "Batch updated {} tasks in {}ms",
            dispatch_updates.len(),
            db_update_ms
        );

        if !failure_updates.is_empty() {
            self.process_status_updates(failure_updates).await?;
        }

        Ok(count)
    }

    pub(super) async fn process_status_updates(&self, updates: Vec<StatusUpdate>) -> Result<()> {
        let mut task_updates: Vec<(Uuid, &str, Option<&str>, Option<&str>)> = Vec::new();
        let mut runs_to_check = std::collections::HashSet::new();
        let mut failed_by_run: HashMap<Uuid, Vec<String>> = HashMap::new();
        let failed_ids: Vec<Uuid> = updates
            .iter()
            .filter(|u| u.status == "failed")
            .map(|u| u.task_id)
            .collect();
        let retry_meta = if failed_ids.is_empty() {
            HashMap::new()
        } else {
            self.db.get_task_retry_meta(&failed_ids).await?
        };

        for update in &updates {
            if let Some(log) = update.log.as_ref()
                && let Err(e) = self.db.append_task_log(update.task_id, log).await
            {
                error!("Failed to append log for task {}: {}", update.task_id, e);
            }
            if let Some(output) = update.output.as_ref() {
                if let Some(deferred) = output.get("deferred").and_then(|v| v.as_array()) {
                    info!(
                        "Task {} returned {} deferrables",
                        update.task_id,
                        deferred.len()
                    );

                    for deferrable_data in deferred {
                        if let (Some(service_type), Some(job_id)) = (
                            deferrable_data.get("service_type").and_then(|v| v.as_str()),
                            deferrable_data.get("job_id").and_then(|v| v.as_str()),
                        ) {
                            match self
                                .db
                                .create_deferred_job(
                                    update.task_id,
                                    service_type,
                                    job_id,
                                    deferrable_data.clone(),
                                )
                                .await
                            {
                                Ok(_) => {
                                    info!(
                                        "Created deferred job for task {}: {} ({})",
                                        update.task_id, job_id, service_type
                                    );
                                }
                                Err(e) => {
                                    error!(
                                        "Failed to create deferred job for task {}: {}",
                                        update.task_id, e
                                    );
                                }
                            }
                        } else {
                            error!(
                                "Invalid deferrable data for task {}: missing service_type or job_id",
                                update.task_id
                            );
                        }
                    }

                    continue;
                }

                if let Err(e) = self
                    .db
                    .update_task_output(update.task_id, output.clone())
                    .await
                {
                    error!("Failed to store output for task {}: {}", update.task_id, e);
                }
            }

            let new_status = match update.status.as_str() {
                "success" => TaskStatus::Success,
                "failed" => TaskStatus::Failed,
                "running" => TaskStatus::Running,
                _ => continue,
            };

            if matches!(new_status, TaskStatus::Failed) {
                let (attempts, max_retries) =
                    retry_meta.get(&update.task_id).cloned().unwrap_or((0, 0));
                if attempts < max_retries {
                    let error = update.error.as_deref().unwrap_or("retrying");
                    let backoff = retry_backoff_seconds(attempts + 1);
                    let retry_at = Utc::now()
                        + chrono::Duration::seconds(i64::try_from(backoff).unwrap_or(i64::MAX));
                    if let Err(e) = self
                        .db
                        .reset_task_for_retry(update.task_id, Some(error), Some(retry_at))
                        .await
                    {
                        error!("Failed to reset task {} for retry: {}", update.task_id, e);
                        task_updates.push((
                            update.task_id,
                            new_status.as_str(),
                            None,
                            update.error.as_deref(),
                        ));
                    } else {
                        continue;
                    }
                }
            }

            task_updates.push((
                update.task_id,
                new_status.as_str(),
                None,
                update.error.as_deref(),
            ));

            if matches!(new_status, TaskStatus::Success | TaskStatus::Failed) {
                let (run_id, task_name) = self.db.get_task_identity(update.task_id).await?;
                runs_to_check.insert(run_id);
                if matches!(new_status, TaskStatus::Failed) {
                    failed_by_run.entry(run_id).or_default().push(task_name);
                }
            }
        }

        if !task_updates.is_empty() {
            self.db.batch_update_task_status(&task_updates).await?;
            info!(
                "Processed {} status updates from executors",
                task_updates.len()
            );
        }

        for (run_id, names) in failed_by_run {
            let mut pending = names;
            let mut seen = HashSet::new();
            while !pending.is_empty() {
                let next = self
                    .db
                    .mark_tasks_failed_by_dependency(run_id, &pending, "dependency failed")
                    .await?;
                pending = next
                    .into_iter()
                    .filter(|name| seen.insert(name.clone()))
                    .collect();
            }
            runs_to_check.insert(run_id);
        }

        for run_id in runs_to_check {
            self.check_run_completion(run_id).await?;
        }

        Ok(())
    }

    pub(super) async fn enforce_timeouts(&self) -> Result<()> {
        let running = self.db.get_running_tasks().await?;
        if running.is_empty() {
            return Ok(());
        }

        let now = Utc::now();
        let mut updates = Vec::new();
        for task in running {
            let timeout = match task.timeout_seconds {
                Some(value) if value > 0 => value,
                _ => continue,
            };
            let started = task
                .started_at
                .or(task.dispatched_at)
                .unwrap_or(task.created_at);
            let elapsed = now.signed_duration_since(started).num_seconds();
            if elapsed > i64::from(timeout) {
                updates.push(StatusUpdate {
                    task_id: task.id,
                    status: "failed".to_string(),
                    log: None,
                    output: None,
                    error: Some(format!("timeout after {}s", timeout)),
                });
            }
        }

        if !updates.is_empty() {
            self.process_status_updates(updates).await?;
        }

        Ok(())
    }

    pub(super) async fn check_run_completion(&self, run_id: Uuid) -> Result<()> {
        let (total, completed, failed) = self.db.get_run_task_stats(run_id).await?;

        if completed == total {
            let new_status = if failed > 0 {
                TaskStatus::Failed
            } else {
                TaskStatus::Success
            };

            self.db
                .update_run_status(run_id, new_status.as_str(), None)
                .await?;
            info!(
                "Run {} completed with status: {} ({}/{} tasks)",
                run_id,
                new_status.as_str(),
                completed,
                total
            );
        }

        Ok(())
    }

    pub(super) async fn process_scheduled_triggers(&self) -> Result<usize> {
        crate::schedule_processor::process_scheduled_triggers(self.db.as_ref()).await
    }

    pub(super) async fn process_job_completions(
        &self,
        completions: Vec<JobCompletionNotification>,
    ) -> Result<()> {
        for completion in completions {
            info!(
                "Processing deferred job completion for task {}: success={}",
                completion.task_id, completion.success
            );

            if completion.success {
                if let Err(e) = self
                    .db
                    .update_task_status(completion.task_id, "success", None, None)
                    .await
                {
                    error!(
                        "Failed to mark task {} as successful: {}",
                        completion.task_id, e
                    );
                    continue;
                }
            } else {
                let error_msg = completion
                    .error
                    .unwrap_or_else(|| "Deferred job failed".to_string());
                if let Err(e) = self
                    .db
                    .update_task_status(completion.task_id, "failed", None, Some(&error_msg))
                    .await
                {
                    error!(
                        "Failed to mark task {} as failed: {}",
                        completion.task_id, e
                    );
                    continue;
                }
            }

            if let Ok(run_id) = self.db.get_task_run_id(completion.task_id).await
                && let Err(e) = self.check_run_completion(run_id).await
            {
                error!("Error checking run completion: {}", e);
            }
        }

        Ok(())
    }
}

Status: Pending Review

# Tutorial: ELT Pipeline with Medallion Architecture

This tutorial shows how to add orchestration to a realistic ELT pipeline with bronze/silver/gold layers.

## Your Existing Code

Let's say you have an ELT pipeline that loads raw data into bronze tables and transforms it to silver:

```py
# src/pipeline.py
import requests
import duckdb
from pydantic import BaseModel, TypeAdapter

class Event(BaseModel):
    id: str
    timestamp: str
    user_id: str
    event_type: str
    raw_data: str

EventList = TypeAdapter(list[Event])

def load_to_bronze(source: str, date: str, db_path: str) -> int:
    """Load raw data from API into bronze table"""
    # Fetch from API (using JSONPlaceholder for example)
    response = requests.get(f"https://jsonplaceholder.typicode.com/{source}")
    response.raise_for_status()
    events = EventList.validate_json(response.text)

    # Load to DuckDB bronze layer
    con = duckdb.connect(db_path)
    con.execute(f"""
        CREATE TABLE IF NOT EXISTS bronze.{source}_events (
            id VARCHAR,
            timestamp VARCHAR,
            user_id VARCHAR,
            event_type VARCHAR,
            raw_data VARCHAR,
            loaded_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
        )
    """)

    con.executemany(
        f"INSERT INTO bronze.{source}_events (id, timestamp, user_id, event_type, raw_data) VALUES (?, ?, ?, ?, ?)",
        [(e.id, e.timestamp, e.user_id, e.event_type, e.raw_data) for e in events]
    )
    con.close()
    return len(events)

def transform_to_silver(source: str, db_path: str) -> int:
    """Transform bronze data to cleaned silver table"""
    con = duckdb.connect(db_path)

    if source == "posts":
        con.execute("""
            CREATE OR REPLACE TABLE silver.posts AS
            SELECT
                CAST(id AS INTEGER) AS post_id,
                CAST(userId AS INTEGER) AS user_id,
                TRIM(title) AS title,
                TRIM(body) AS body,
                loaded_at
            FROM bronze.posts
            WHERE id IS NOT NULL
        """)
    elif source == "comments":
        con.execute("""
            CREATE OR REPLACE TABLE silver.comments AS
            SELECT
                CAST(id AS INTEGER) AS comment_id,
                CAST(postId AS INTEGER) AS post_id,
                TRIM(name) AS name,
                LOWER(TRIM(email)) AS email,
                TRIM(body) AS body,
                loaded_at
            FROM bronze.comments
            WHERE id IS NOT NULL
        """)
    elif source == "users":
        con.execute("""
            CREATE OR REPLACE TABLE silver.users AS
            SELECT
                CAST(id AS INTEGER) AS user_id,
                TRIM(name) AS name,
                TRIM(username) AS username,
                LOWER(TRIM(email)) AS email,
                loaded_at
            FROM bronze.users
            WHERE id IS NOT NULL
        """)

    result = con.execute(f"SELECT COUNT(*) FROM silver.{source}").fetchone()
    con.close()
    return result[0]

def create_gold_analytics(db_path: str) -> dict:
    """Create gold layer with analytics"""
    con = duckdb.connect(db_path)

    # User engagement metrics
    con.execute("""
        CREATE OR REPLACE TABLE gold.user_engagement AS
        SELECT
            u.user_id,
            u.name,
            u.username,
            COUNT(DISTINCT p.post_id) AS post_count,
            COUNT(DISTINCT c.comment_id) AS comment_count,
            COUNT(DISTINCT c.comment_id) * 1.0 / NULLIF(COUNT(DISTINCT p.post_id), 0) AS avg_comments_per_post
        FROM silver.users u
        LEFT JOIN silver.posts p ON u.user_id = p.user_id
        LEFT JOIN silver.comments c ON p.post_id = c.post_id
        GROUP BY u.user_id, u.name, u.username
    """)

    # Most discussed posts
    con.execute("""
        CREATE OR REPLACE TABLE gold.top_posts AS
        SELECT
            p.post_id,
            p.title,
            u.name AS author,
            COUNT(c.comment_id) AS comment_count
        FROM silver.posts p
        JOIN silver.users u ON p.user_id = u.user_id
        LEFT JOIN silver.comments c ON p.post_id = c.post_id
        GROUP BY p.post_id, p.title, u.name
        ORDER BY comment_count DESC
        LIMIT 10
    """)

    # Summary stats
    stats = con.execute("""
        SELECT
            (SELECT COUNT(*) FROM silver.users) AS total_users,
            (SELECT COUNT(*) FROM silver.posts) AS total_posts,
            (SELECT COUNT(*) FROM silver.comments) AS total_comments,
            (SELECT AVG(comment_count) FROM gold.user_engagement) AS avg_comments_per_user
    """).fetchone()

    con.close()

    return {
        "total_users": stats[0],
        "total_posts": stats[1],
        "total_comments": stats[2],
        "avg_comments_per_user": round(stats[3], 2)
    }
```

You run this as a script for each source:

```py
DB_PATH = "data/warehouse.db"

for source in ["posts", "comments", "users"]:
    bronze_count = load_to_bronze(source, "2024-01-15", DB_PATH)
    print(f"Loaded {bronze_count} records to bronze.{source}")

    silver_count = transform_to_silver(source, DB_PATH)
    print(f"Transformed {silver_count} records to silver.{source}")

# After all sources loaded and transformed
stats = create_gold_analytics(DB_PATH)
print(f"Analytics: {stats['total_users']} users, {stats['total_posts']} posts, {stats['total_comments']} comments")
```

**Problems**:
- No error handling or retries (API failures break everything)
- Runs sequentially (could load all sources in parallel)
- No scheduling (run manually or via cron)
- Hard to monitor which sources succeeded/failed
- Bronze load failure prevents silver transformation

## Add Orchestration

### 1. Create Task Wrappers

Task wrappers connect your existing functions to Ork. Each wrapper defines:
- What inputs the task expects
- What outputs it produces
- How to call your existing function

#### Bronze Task

```py
# tasks/load_bronze.py
from ork import task
from pydantic import BaseModel
from src.pipeline import load_to_bronze

class Input(BaseModel):
    source: str
    date: str
    db_path: str

class Output(BaseModel):
    source: str
    count: int

@task
def main(input: Input) -> Output:
    count = load_to_bronze(input.source, input.date, input.db_path)
    return Output(source=input.source, count=count)
```

**What this does**:
- `Input` defines what the task needs (source name, date, database path)
- `Output` defines what the task produces (source name and record count)
- `main()` calls your existing `load_to_bronze()` function
- The `@task` decorator makes this runnable by Ork

#### Silver Task

```py
# tasks/transform_silver.py
from ork import task
from pydantic import BaseModel
from src.pipeline import transform_to_silver

class BronzeOutput(BaseModel):
    source: str
    count: int

class Input(BaseModel):
    bronze: BronzeOutput  # ← Receives output from bronze task
    db_path: str

class Output(BaseModel):
    source: str
    bronze_count: int
    silver_count: int

@task
def main(input: Input) -> Output:
    silver_count = transform_to_silver(input.bronze.source, input.db_path)
    return Output(
        source=input.bronze.source,
        bronze_count=input.bronze.count,
        silver_count=silver_count
    )
```

**What this does**:
- `Input` declares a field named `bronze` - this tells Ork to pass the output from the bronze task
- `BronzeOutput` defines what we expect from bronze (matching its `Output` class)
- Also receives `db_path` as a static input
- `main()` accesses the source via `input.bronze.source` and calls your `transform_to_silver()` function

**Key point**: The field name `bronze` matches the task name in the workflow. That's how Ork knows which output to pass.

**Your business logic didn't change** - we just wrapped it with input/output definitions.

#### Gold Analytics Task

```py
# tasks/gold_analytics.py
from ork import task
from pydantic import BaseModel
from src.pipeline import create_gold_analytics

class SilverOutput(BaseModel):
    source: str
    bronze_count: int
    silver_count: int

class Input(BaseModel):
    silver_posts: SilverOutput
    silver_comments: SilverOutput
    silver_users: SilverOutput
    db_path: str

class Output(BaseModel):
    total_users: int
    total_posts: int
    total_comments: int
    avg_comments_per_user: float

@task
def main(input: Input) -> Output:
    stats = create_gold_analytics(input.db_path)
    return Output(**stats)
```

**What this does**:
- Waits for ALL three silver tables to complete
- `Input` declares fields for all three upstream silver tasks
- Creates gold layer analytics tables (user engagement, top posts)
- Returns summary statistics

### 2. Define the Workflow

The workflow file tells Ork how to run your tasks:

```yaml
# workflows/elt_pipeline.yaml
name: elt_pipeline

tasks:
  bronze_posts:
    executor: python
    file: tasks/load_bronze.py
    input:
      source: posts
      date: "2024-01-15"
      db_path: data/warehouse.db
    timeout: 600
    retries: 3

  bronze_comments:
    executor: python
    file: tasks/load_bronze.py
    input:
      source: comments
      date: "2024-01-15"
      db_path: data/warehouse.db
    timeout: 600
    retries: 3

  bronze_users:
    executor: python
    file: tasks/load_bronze.py
    input:
      source: users
      date: "2024-01-15"
      db_path: data/warehouse.db
    timeout: 600
    retries: 3

  silver_posts:
    executor: python
    file: tasks/transform_silver.py
    input:
      db_path: data/warehouse.db
    depends_on: [bronze_posts]

  silver_comments:
    executor: python
    file: tasks/transform_silver.py
    input:
      db_path: data/warehouse.db
    depends_on: [bronze_comments]

  silver_users:
    executor: python
    file: tasks/transform_silver.py
    input:
      db_path: data/warehouse.db
    depends_on: [bronze_users]

  gold_analytics:
    executor: python
    file: tasks/gold_analytics.py
    input:
      db_path: data/warehouse.db
    depends_on: [silver_posts, silver_comments, silver_users]
```

**Breaking this down**:

- `name: elt_pipeline` - Unique identifier for this workflow
- `tasks:` - List of all tasks in the workflow

**For each task**:
- `executor: python` - Run this task using Python
- `file: tasks/load_bronze.py` - Path to the task file (reused for all bronze tasks)
- `input:` - Static input values for the task (source, date, database path)
- `timeout: 600` - Kill the task if it runs longer than 10 minutes
- `retries: 3` - If the task fails, retry up to 3 times
- `depends_on: [bronze_app]` - Don't run this task until `bronze_app` completes successfully

**Execution order**:
1. All three `bronze_*` tasks run **in parallel** - loading raw data from JSONPlaceholder API
2. Each `silver_*` task runs after its bronze task succeeds - cleaning and normalizing in the database
3. Silver tasks run in parallel (independent of each other)
4. `gold_analytics` waits for ALL silver tasks to complete, then creates analytics tables

This is **ELT with medallion architecture**:
- **Bronze** (raw): Fetch from API (posts, comments, users) and load as-is into DuckDB
- **Silver** (cleaned): Clean, normalize, and type-cast the data using SQL
- **Gold** (analytics): Join tables and compute metrics:
  - User engagement (posts per user, comments per post)
  - Top posts by comment count
  - Summary statistics

If any bronze load fails, only its corresponding silver transformation is skipped. The gold analytics task only runs if all three silver tables succeed.

### 3. What You Get

Now Ork handles:
- **Parallel execution** - Bronze loads and silver transformations run in parallel
- **Complex dependencies** - Gold analytics waits for all three silver tables
- **Retries** - Failed API calls or database operations retry automatically
- **Partial failures** - If one source fails, others still process (analytics skipped if any silver fails)
- **Scheduling** - Add `schedule: "0 2 * * *"` to run daily at 2am
- **Monitoring** - See which layers succeeded/failed, view logs per task
- **Scalability** - Each task can run on a different container/machine
- **Low idle cost** - When deployed serverless, costs ~$0 when workflows aren't running

Your code stays the same. Ork just coordinates when and where it runs.

**What you built**: A real 3-layer ELT pipeline (bronze → silver → gold) that:
- Ingests posts, comments, and users from JSONPlaceholder API in parallel
- Cleans and normalizes data in silver tables
- Computes analytics: user engagement metrics, top posts, and summary stats

**Note**: This example uses explicit tasks per source. For real workflows with many sources, you'd use `foreach` - see [Parallelization](parallelization.md).

## Key Takeaways

1. **Keep your business logic separate** - The functions in `src/pipeline.py` have no knowledge of Ork
2. **Task wrappers define inputs/outputs** - Tasks receive outputs from upstream tasks via `input.<task_name>`
3. **Workflows define execution order** - Use `depends_on` to create a DAG
4. **Ork handles orchestration** - Retries, scheduling, monitoring, scaling

## Next Steps

- [Workflows](workflows.md) - DAG definitions and dependencies
- [Tasks](tasks.md) - Writing task wrappers in detail
- [Parallelization](parallelization.md) - Process data in parallel
- [Operations](operations.md) - Running and scheduling workflows

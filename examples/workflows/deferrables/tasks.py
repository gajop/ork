#!/usr/bin/env python3
"""
Example workflow demonstrating deferrables with Ork.

This example shows how to use deferrables to track long-running external jobs
without keeping the worker container running.
"""

import json
import os
import sys

# Add python-sdk to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '../../../python-sdk'))

from ork_sdk import BigQueryJob, CustomHttp


def simulate_bigquery_job():
    """
    Simulates starting a BigQuery job and returning a deferrable.

    In a real scenario, this would:
    1. Use google-cloud-bigquery to start a query
    2. Get the job_id from the BigQuery API
    3. Return BigQueryJob deferrable

    The worker then scales to 0, and the scheduler tracks the job.
    """
    # Simulate BigQuery job details
    project = os.getenv("GCP_PROJECT", "my-project")
    job_id = f"simulated_job_{os.getpid()}"
    location = "US"

    print(f"Starting BigQuery job: {job_id}")
    print(f"Project: {project}, Location: {location}")

    # Return deferrable - tells Ork to track this job
    deferrable = BigQueryJob(
        project=project,
        job_id=job_id,
        location=location
    )

    # Return as JSON with special structure
    result = {
        "deferred": [deferrable.to_dict()]
    }

    print(f"ORK_OUTPUT:{json.dumps(result)}")
    return result


def simulate_custom_http_job():
    """
    Simulates a custom HTTP job that polls a status endpoint.

    This is useful for tracking jobs from custom APIs that provide
    status endpoints.
    """
    job_id = f"custom_job_{os.getpid()}"
    status_url = f"https://api.example.com/jobs/{job_id}/status"

    print(f"Starting custom HTTP job: {job_id}")
    print(f"Status URL: {status_url}")

    # Return CustomHttp deferrable
    deferrable = CustomHttp(
        url=status_url,
        method="GET",
        headers={"Authorization": "Bearer fake-token"},
        completion_field="status",
        completion_value="completed",
        failure_value="failed"
    )

    result = {
        "deferred": [deferrable.to_dict()]
    }

    print(f"ORK_OUTPUT:{json.dumps(result)}")
    return result


def multiple_jobs():
    """
    Example of returning multiple deferrables.

    The task completes only when ALL deferred jobs complete.
    """
    project = os.getenv("GCP_PROJECT", "my-project")

    # Start multiple jobs
    bq_job = BigQueryJob(
        project=project,
        job_id=f"analytics_job_{os.getpid()}",
        location="US"
    )

    ml_job = BigQueryJob(
        project=project,
        job_id=f"ml_training_{os.getpid()}",
        location="EU"
    )

    print("Starting multiple BigQuery jobs")

    result = {
        "deferred": [
            bq_job.to_dict(),
            ml_job.to_dict()
        ]
    }

    print(f"ORK_OUTPUT:{json.dumps(result)}")
    return result


def process_results(input_data):
    """
    Example of a downstream task that runs after deferred jobs complete.

    This task would fetch and process the results from the BigQuery tables
    that were populated by the previous task's deferred job.
    """
    upstream = input_data.get("upstream", {})

    print("Processing results from upstream deferred jobs")
    print(f"Upstream data: {json.dumps(upstream, indent=2)}")

    result = {
        "message": "Results processed successfully",
        "upstream_tasks": list(upstream.keys())
    }

    print(f"ORK_OUTPUT:{json.dumps(result)}")
    return result


if __name__ == "__main__":
    # Read input from environment variable
    input_json = os.getenv("ORK_INPUT_JSON", "{}")
    task_input = json.loads(input_json) if input_json else {}

    # Get task name from params
    task_name = task_input.get("task_name", "simulate_bigquery_job")

    # Execute the appropriate task
    if task_name == "simulate_bigquery_job":
        simulate_bigquery_job()
    elif task_name == "simulate_custom_http_job":
        simulate_custom_http_job()
    elif task_name == "multiple_jobs":
        multiple_jobs()
    elif task_name == "process_results":
        process_results(task_input)
    else:
        print(f"Unknown task: {task_name}")
        sys.exit(1)

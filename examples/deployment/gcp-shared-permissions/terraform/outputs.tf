output "scheduler_url" {
  description = "Ork Scheduler Cloud Run URL"
  value       = google_cloud_run_v2_service.scheduler.uri
}

output "api_url" {
  description = "Ork API Cloud Run URL"
  value       = google_cloud_run_v2_service.api.uri
}

output "bucket_name" {
  description = "GCS bucket name for Ork state"
  value       = google_storage_bucket.ork.name
}

output "artifact_registry_url" {
  description = "Artifact Registry repository URL"
  value       = "${var.region}-docker.pkg.dev/${var.project}/${google_artifact_registry_repository.workflows.repository_id}"
}

output "worker_service_account" {
  description = "Worker service account email"
  value       = google_service_account.worker.email
}

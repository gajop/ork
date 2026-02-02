output "scheduler_url" {
  description = "Ork Scheduler Cloud Run URL"
  value       = google_cloud_run_v2_service.scheduler.uri
}

output "api_url" {
  description = "Ork API Cloud Run URL"
  value       = google_cloud_run_v2_service.api.uri
}

output "ork_bucket_name" {
  description = "GCS bucket name for Ork state"
  value       = google_storage_bucket.ork.name
}

output "raw_bucket_name" {
  description = "GCS bucket name for raw data"
  value       = google_storage_bucket.raw.name
}

output "artifact_registry_url" {
  description = "Artifact Registry repository URL"
  value       = "${var.region}-docker.pkg.dev/${var.project}/${google_artifact_registry_repository.workflows.repository_id}"
}

output "extract_service_account" {
  description = "Extract worker service account email"
  value       = google_service_account.extract.email
}

output "transform_service_account" {
  description = "Transform worker service account email"
  value       = google_service_account.transform.email
}

output "load_service_account" {
  description = "Load worker service account email"
  value       = google_service_account.load.email
}

# Scheduler service account
resource "google_service_account" "scheduler" {
  account_id   = "ork-scheduler"
  display_name = "Ork Scheduler"
}

resource "google_project_iam_member" "scheduler" {
  for_each = toset([
    "roles/datastore.user",
    "roles/run.developer",
  ])
  project = var.project
  role    = each.value
  member  = "serviceAccount:${google_service_account.scheduler.email}"
}

resource "google_storage_bucket_iam_member" "scheduler" {
  bucket = google_storage_bucket.ork.name
  role   = "roles/storage.objectAdmin"
  member = "serviceAccount:${google_service_account.scheduler.email}"
}

# Worker service account with shared permissions
resource "google_service_account" "worker" {
  account_id   = "ork-worker"
  display_name = "Ork Worker"
}

resource "google_project_iam_member" "worker" {
  for_each = toset([
    "roles/storage.objectAdmin",
    "roles/bigquery.dataEditor",
    "roles/bigquery.jobUser",
  ])
  project = var.project
  role    = each.value
  member  = "serviceAccount:${google_service_account.worker.email}"
}

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

# Extract worker: external API access, write to GCS raw bucket only
resource "google_service_account" "extract" {
  account_id   = "extract-worker"
  display_name = "Extract Worker"
}

resource "google_storage_bucket_iam_member" "extract_raw" {
  bucket = google_storage_bucket.raw.name
  role   = "roles/storage.objectCreator"
  member = "serviceAccount:${google_service_account.extract.email}"
}

# Transform worker: BigQuery only, no GCS, no internet
resource "google_service_account" "transform" {
  account_id   = "transform-worker"
  display_name = "Transform Worker"
}

resource "google_project_iam_member" "transform_bq" {
  for_each = toset([
    "roles/bigquery.dataEditor",
    "roles/bigquery.jobUser",
  ])
  project = var.project
  role    = each.value
  member  = "serviceAccount:${google_service_account.transform.email}"
}

# Load worker: read GCS, write to BigQuery
resource "google_service_account" "load" {
  account_id   = "load-worker"
  display_name = "Load Worker"
}

resource "google_storage_bucket_iam_member" "load_raw" {
  bucket = google_storage_bucket.raw.name
  role   = "roles/storage.objectViewer"
  member = "serviceAccount:${google_service_account.load.email}"
}

resource "google_project_iam_member" "load_bq" {
  for_each = toset([
    "roles/bigquery.dataEditor",
    "roles/bigquery.jobUser",
  ])
  project = var.project
  role    = each.value
  member  = "serviceAccount:${google_service_account.load.email}"
}

# Firestore database
resource "google_firestore_database" "ork" {
  name        = "ork"
  location_id = var.region
  type        = "FIRESTORE_NATIVE"
}

# GCS bucket for Ork state
resource "google_storage_bucket" "ork" {
  name     = "ork-${var.project}"
  location = var.region

  uniform_bucket_level_access = true
}

# GCS bucket for raw data
resource "google_storage_bucket" "raw" {
  name     = "ork-raw-${var.project}"
  location = var.region

  uniform_bucket_level_access = true
}

# Upload worker config
resource "google_storage_bucket_object" "workers_config" {
  bucket = google_storage_bucket.ork.name
  name   = "config/workers.yaml"
  source = "${path.module}/../workers.yaml"
}

# Upload code locations config
resource "google_storage_bucket_object" "locations_config" {
  bucket = google_storage_bucket.ork.name
  name   = "config/locations.yaml"
  source = "${path.module}/../locations.yaml"
}

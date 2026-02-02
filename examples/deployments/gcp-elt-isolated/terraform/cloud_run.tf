# Ork Scheduler
resource "google_cloud_run_v2_service" "scheduler" {
  name     = "ork-scheduler"
  location = var.region

  template {
    service_account = google_service_account.scheduler.email

    containers {
      image = var.ork_scheduler_image

      env {
        name  = "ORK_WORKERS_CONFIG"
        value = "gs://${google_storage_bucket.ork.name}/config/workers.yaml"
      }

      env {
        name  = "ORK_LOCATIONS_CONFIG"
        value = "gs://${google_storage_bucket.ork.name}/config/locations.yaml"
      }

      env {
        name  = "ORK_STATE_STORE"
        value = "firestore"
      }

      env {
        name  = "ORK_OBJECT_STORE"
        value = "gs://${google_storage_bucket.ork.name}"
      }

      env {
        name  = "GCP_PROJECT"
        value = var.project
      }
    }
  }

  depends_on = [
    google_storage_bucket_object.workers_config,
    google_storage_bucket_object.locations_config,
  ]
}

# Ork API
resource "google_cloud_run_v2_service" "api" {
  name     = "ork-api"
  location = var.region

  template {
    service_account = google_service_account.scheduler.email

    containers {
      image = var.ork_api_image

      env {
        name  = "ORK_STATE_STORE"
        value = "firestore"
      }

      env {
        name  = "ORK_OBJECT_STORE"
        value = "gs://${google_storage_bucket.ork.name}"
      }

      env {
        name  = "GCP_PROJECT"
        value = var.project
      }
    }
  }
}

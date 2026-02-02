# Artifact Registry for workflow images
resource "google_artifact_registry_repository" "workflows" {
  repository_id = "workflows"
  format        = "DOCKER"
  location      = var.region
  description   = "Ork workflow images"
}

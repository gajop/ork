variable "project" {
  description = "GCP project ID"
  type        = string
}

variable "region" {
  description = "GCP region"
  type        = string
  default     = "us-central1"
}

variable "ork_scheduler_image" {
  description = "Ork scheduler container image"
  type        = string
  default     = "ork/scheduler:0.1.0"
}

variable "ork_api_image" {
  description = "Ork API container image"
  type        = string
  default     = "ork/api:0.1.0"
}

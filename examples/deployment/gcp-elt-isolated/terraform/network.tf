# VPC network
resource "google_compute_network" "main" {
  name                    = "main"
  auto_create_subnetworks = false
}

# Subnet
resource "google_compute_subnetwork" "main" {
  name          = "main-subnet"
  ip_cidr_range = "10.0.0.0/24"
  region        = var.region
  network       = google_compute_network.main.id
}

# VPC connector for external access (with internet)
resource "google_vpc_access_connector" "external" {
  name          = "external"
  region        = var.region
  network       = google_compute_network.main.name
  ip_cidr_range = "10.8.0.0/28"
}

# VPC connector for internal access (no internet)
resource "google_vpc_access_connector" "internal" {
  name          = "internal"
  region        = var.region
  network       = google_compute_network.main.name
  ip_cidr_range = "10.9.0.0/28"
}

# Firewall rule to block egress for internal connector
resource "google_compute_firewall" "deny_internal_egress" {
  name    = "deny-internal-egress"
  network = google_compute_network.main.name

  deny {
    protocol = "all"
  }

  direction          = "EGRESS"
  destination_ranges = ["0.0.0.0/0"]
  target_tags        = ["internal"]
}

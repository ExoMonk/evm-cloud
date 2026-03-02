variable "host_address" {
  description = "IP or hostname of the target host."
  type        = string
}

variable "ssh_user" {
  description = "SSH user for the target host."
  type        = string
  default     = "ubuntu"
}

variable "ssh_private_key_path" {
  description = "Path to SSH private key file."
  type        = string
}

variable "ssh_port" {
  description = "SSH port."
  type        = number
  default     = 22
}

variable "k3s_version" {
  description = "k3s version to install (e.g., v1.30.4+k3s1)."
  type        = string
  default     = "v1.30.4+k3s1"
}

variable "tls_san_entries" {
  description = "Additional TLS SAN entries for the k3s API server certificate (IPs or hostnames)."
  type        = list(string)
  default     = []
}

variable "project_name" {
  description = "Project name used for k3s node naming."
  type        = string
}

variable "cluster_cidr" {
  description = "Pod network CIDR for k3s. Must not overlap with the host VPC/subnet CIDR."
  type        = string
  default     = "10.244.0.0/16"
}

variable "service_cidr" {
  description = "Service network CIDR for k3s. Must not overlap with the host VPC/subnet CIDR."
  type        = string
  default     = "10.245.0.0/16"
}

variable "extra_server_flags" {
  description = "Additional flags to pass to k3s server install."
  type        = string
  default     = ""
}

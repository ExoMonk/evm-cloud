output "config_dir" {
  description = "Path to config directory on VPS."
  value       = "/opt/evm-cloud/config"
}

output "compose_file" {
  description = "Path to docker-compose.yml on VPS."
  value       = "/opt/evm-cloud/docker-compose.yml"
}

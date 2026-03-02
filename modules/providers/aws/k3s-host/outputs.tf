output "host_ip" {
  description = "Public IP of the k3s host."
  value       = aws_instance.k3s.public_ip
}

output "instance_id" {
  description = "EC2 instance ID."
  value       = aws_instance.k3s.id
}

output "ssh_user" {
  description = "SSH user for the k3s host."
  value       = "ubuntu"
}

output "security_group_id" {
  description = "Security group ID for the k3s host."
  value       = aws_security_group.k3s.id
}

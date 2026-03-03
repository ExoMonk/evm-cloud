output "host_ip" {
  description = "Public IP of the k3s host."
  value       = var.use_spot ? aws_spot_instance_request.k3s[0].public_ip : aws_instance.k3s[0].public_ip
}

output "instance_id" {
  description = "EC2 instance ID."
  value       = var.use_spot ? aws_spot_instance_request.k3s[0].spot_instance_id : aws_instance.k3s[0].id
}

output "ssh_user" {
  description = "SSH user for the k3s host."
  value       = "ubuntu"
}

output "security_group_id" {
  description = "Security group ID for the k3s host."
  value       = aws_security_group.k3s.id
}

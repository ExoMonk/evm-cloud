output "instance_id" {
  description = "EC2 instance ID."
  value       = aws_instance.this.id
}

output "instance_public_ip" {
  description = "EC2 instance public IP address."
  value       = aws_instance.this.public_ip
}

output "instance_private_ip" {
  description = "EC2 instance private IP address."
  value       = aws_instance.this.private_ip
}

output "ssh_command" {
  description = "SSH command to connect to the instance."
  value       = "ssh -i <key-file> ec2-user@${aws_instance.this.public_ip}"
}

output "cloudwatch_log_group" {
  description = "CloudWatch log group name for container logs."
  value       = aws_cloudwatch_log_group.services.name
}

output "secret_arn" {
  description = "Secrets Manager secret ARN for the .env payload."
  value       = aws_secretsmanager_secret.env.arn
}

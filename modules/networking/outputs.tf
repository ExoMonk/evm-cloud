output "vpc_id" {
  description = "VPC ID for downstream modules."
  value       = aws_vpc.this.id
}

output "public_subnet_ids" {
  description = "Public subnet IDs."
  value       = aws_subnet.public[*].id
}

output "private_subnet_ids" {
  description = "Private subnet IDs."
  value       = aws_subnet.private[*].id
}

output "security_group_ids" {
  description = "Security group IDs keyed by service role."
  value = {
    alb        = aws_security_group.alb.id
    erpc       = aws_security_group.erpc.id
    indexer    = aws_security_group.indexer.id
    database   = aws_security_group.database.id
    monitoring = aws_security_group.monitoring.id
  }
}

output "role_arn" {
  description = "ARN of the IRSA role for ESO to assume via ServiceAccount annotation."
  value       = aws_iam_role.eso.arn
}

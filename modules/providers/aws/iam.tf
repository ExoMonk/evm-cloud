# EC2 instance role for Docker Compose compute engine.
# Grants: CloudWatch Logs write + Secrets Manager read for the project's env secret.

data "aws_caller_identity" "current" {}

data "aws_iam_policy_document" "ec2_assume_role" {
  statement {
    effect = "Allow"
    principals {
      type        = "Service"
      identifiers = ["ec2.amazonaws.com"]
    }
    actions = ["sts:AssumeRole"]
  }
}

resource "aws_iam_role" "ec2_instance" {
  count = local.any_ec2_compute ? 1 : 0

  name               = "${var.project_name}-${var.network_environment}-ec2-instance"
  assume_role_policy = data.aws_iam_policy_document.ec2_assume_role.json
  tags               = local.common_tags
}

resource "aws_iam_role_policy" "ec2_cloudwatch_logs" {
  count = local.any_ec2_compute ? 1 : 0

  name = "${var.project_name}-${var.network_environment}-ec2-cloudwatch-logs"
  role = aws_iam_role.ec2_instance[0].id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "logs:CreateLogStream",
          "logs:PutLogEvents"
        ]
        Resource = "arn:aws:logs:${var.aws_region}:${data.aws_caller_identity.current.account_id}:log-group:/evm-cloud/${var.project_name}-${var.network_environment}:*"
      }
    ]
  })
}

resource "aws_iam_role_policy" "ec2_secrets_manager" {
  count = local.any_ec2_compute ? 1 : 0

  name = "${var.project_name}-${var.network_environment}-ec2-secrets-manager"
  role = aws_iam_role.ec2_instance[0].id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = [
      {
        Effect = "Allow"
        Action = [
          "secretsmanager:GetSecretValue"
        ]
        Resource = "arn:aws:secretsmanager:${var.aws_region}:${data.aws_caller_identity.current.account_id}:secret:evm-cloud/${var.project_name}/*"
      }
    ]
  })
}

resource "aws_iam_instance_profile" "ec2" {
  count = local.any_ec2_compute ? 1 : 0

  name = "${var.project_name}-${var.network_environment}-ec2"
  role = aws_iam_role.ec2_instance[0].name
  tags = local.common_tags
}

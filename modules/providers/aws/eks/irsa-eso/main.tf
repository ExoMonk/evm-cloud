# IRSA role for External Secrets Operator (ESO) on EKS.
# Allows ESO pods to read AWS Secrets Manager secrets via OIDC federation.

data "aws_iam_openid_connect_provider" "eks" {
  arn = var.oidc_provider_arn
}

locals {
  # OIDC issuer URL without the https:// prefix (used in trust policy conditions)
  oidc_issuer = replace(data.aws_iam_openid_connect_provider.eks.url, "https://", "")
}

resource "aws_iam_role" "eso" {
  name = "${var.project_name}-eso-irsa"

  assume_role_policy = jsonencode({
    Version = "2012-10-17"
    Statement = [{
      Effect = "Allow"
      Principal = {
        Federated = var.oidc_provider_arn
      }
      Action = "sts:AssumeRoleWithWebIdentity"
      Condition = {
        StringEquals = {
          "${local.oidc_issuer}:sub" = "system:serviceaccount:${var.eso_namespace}:${var.eso_service_account_name}"
          "${local.oidc_issuer}:aud" = "sts.amazonaws.com"
        }
      }
    }]
  })

  tags = {
    "evm-cloud/component" = "eso-irsa"
    "evm-cloud/project"   = var.project_name
  }
}

resource "aws_iam_role_policy" "eso_sm_read" {
  name = "${var.project_name}-eso-sm-read"
  role = aws_iam_role.eso.id

  policy = jsonencode({
    Version = "2012-10-17"
    Statement = concat(
      [{
        Effect   = "Allow"
        Action   = ["secretsmanager:GetSecretValue"]
        Resource = var.secret_arns
      }],
      var.kms_key_arn != "" ? [{
        Effect   = "Allow"
        Action   = ["kms:Decrypt"]
        Resource = [var.kms_key_arn]
      }] : []
    )
  })
}

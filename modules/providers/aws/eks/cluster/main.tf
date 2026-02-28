module "eks" {
  #checkov:skip=CKV_TF_1:Registry version pins are standard for community modules
  #checkov:skip=CKV_AWS_39:EKS public endpoint acceptable for Tier 0
  #checkov:skip=CKV_AWS_58:EKS secrets encryption optional for Tier 0
  source  = "terraform-aws-modules/eks/aws"
  version = "~> 21.0"

  name               = "${var.project_name}-${var.environment}"
  kubernetes_version = var.kubernetes_version

  vpc_id     = var.vpc_id
  subnet_ids = var.subnet_ids

  # OIDC provider for IAM Roles for Service Accounts (IRSA)
  enable_irsa = true

  # Public endpoint for simplicity in Tier 0
  endpoint_public_access = true

  eks_managed_node_groups = {
    default = {
      instance_types = [var.node_instance_type]
      min_size       = var.node_min_size
      max_size       = var.node_max_size
      desired_size   = var.node_desired_size
    }
  }

  tags = var.common_tags

  timeouts = {
    create = "30m"
    update = null
    delete = "15m"
  }
}

# S3 bucket for service config files (erpc.yaml, rindexer.yaml, ABIs).
# ECS tasks pull config from S3 at boot via entrypoint scripts.

resource "aws_s3_bucket" "config" {
  #checkov:skip=CKV_AWS_144:Cross-region replication not needed for config bucket
  #checkov:skip=CKV_AWS_145:SSE-S3 is default; KMS optional for Tier 0
  #checkov:skip=CKV2_AWS_62:Event notifications not needed for config bucket
  #checkov:skip=CKV_AWS_18:Access logging not needed for config bucket
  #checkov:skip=CKV2_AWS_61:Lifecycle configuration not needed for config bucket
  count = local.any_ecs_compute ? 1 : 0

  bucket = "${var.project_name}-${var.network_environment}-config"

  tags = merge(local.common_tags, { Name = "${var.project_name}-${var.network_environment}-config" })
}

resource "aws_s3_bucket_versioning" "config" {
  count  = local.any_ecs_compute ? 1 : 0
  bucket = aws_s3_bucket.config[0].id

  versioning_configuration {
    status = "Enabled"
  }
}

resource "aws_s3_bucket_public_access_block" "config" {
  count  = local.any_ecs_compute ? 1 : 0
  bucket = aws_s3_bucket.config[0].id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

# --- eRPC config ---

resource "aws_s3_object" "erpc_config" {
  count = (var.rpc_proxy_enabled && var.compute_engine == "ecs" && local.terraform_manages_workloads) ? 1 : 0

  bucket  = aws_s3_bucket.config[0].id
  key     = "erpc/erpc.yaml"
  content = var.erpc_config_yaml

  tags = local.common_tags
}

# --- rindexer config ---

resource "aws_s3_object" "rindexer_config" {
  count = (var.indexer_enabled && var.compute_engine == "ecs" && local.terraform_manages_workloads) ? 1 : 0

  bucket  = aws_s3_bucket.config[0].id
  key     = "rindexer/rindexer.yaml"
  content = var.rindexer_config_yaml

  tags = local.common_tags
}

resource "aws_s3_object" "rindexer_abis" {
  for_each = (var.indexer_enabled && var.compute_engine == "ecs" && local.terraform_manages_workloads) ? var.rindexer_abis : {}

  bucket  = aws_s3_bucket.config[0].id
  key     = "rindexer/abis/${each.key}"
  content = each.value

  tags = local.common_tags
}

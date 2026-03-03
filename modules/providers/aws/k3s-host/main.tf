# AWS k3s Host — dedicated EC2 instance for k3s single-node cluster.
# Separate from ec2/ module: different SG rules (6443 vs compose), no Docker Compose coupling.

locals {
  # Always include VPC CIDR (worker nodes need 6443 + SSH to server).
  # User-provided CIDRs are merged in for external access (kubectl, SSH from home IP).
  api_allowed_cidrs = distinct(concat(
    var.k3s_api_allowed_cidrs,
    [var.vpc_cidr]
  ))
}

# --- AMI: Ubuntu 22.04 LTS (k3s prefers Ubuntu/Debian) ---

data "aws_ami" "ubuntu" {
  most_recent = true
  owners      = ["099720109477"] # Canonical

  filter {
    name   = "name"
    values = ["ubuntu/images/hvm-ssd/ubuntu-jammy-22.04-amd64-server-*"]
  }

  filter {
    name   = "virtualization-type"
    values = ["hvm"]
  }
}

# --- SSH Key Pair ---

resource "aws_key_pair" "k3s" {
  key_name   = "${var.project_name}-${var.environment}-k3s"
  public_key = var.ssh_public_key
  tags       = var.tags
}

# --- Security Group ---

#checkov:skip=CKV2_AWS_5:Security group is attached to k3s EC2 instance
resource "aws_security_group" "k3s" {
  #checkov:skip=CKV_AWS_260:SSH access scoped to VPC CIDR by default
  #checkov:skip=CKV_AWS_24:SSH CIDRs are user-controlled via k3s_api_allowed_cidrs (default: VPC CIDR only)
  name_prefix = "${var.project_name}-k3s-"
  description = "Security group for k3s host"
  vpc_id      = var.vpc_id
  tags        = merge(var.tags, { Name = "${var.project_name}-k3s" })

  # SSH access (same scope as k3s API)
  ingress {
    description = "SSH"
    from_port   = 22
    to_port     = 22
    protocol    = "tcp"
    cidr_blocks = local.api_allowed_cidrs
  }

  # k3s API server — restricted to allowed CIDRs (defaults to VPC CIDR)
  ingress {
    description = "k3s API"
    from_port   = 6443
    to_port     = 6443
    protocol    = "tcp"
    cidr_blocks = local.api_allowed_cidrs
  }

  # Flannel VXLAN — cross-node pod networking (required for multi-node clusters)
  ingress {
    description = "Flannel VXLAN"
    from_port   = 8472
    to_port     = 8472
    protocol    = "udp"
    cidr_blocks = [var.vpc_cidr]
  }

  # Kubelet metrics — required for kubectl logs/exec and metrics-server
  ingress {
    description = "Kubelet metrics"
    from_port   = 10250
    to_port     = 10250
    protocol    = "tcp"
    cidr_blocks = [var.vpc_cidr]
  }

  # NodePort range — restricted to VPC CIDR
  ingress {
    description = "NodePort services"
    from_port   = 30000
    to_port     = 32767
    protocol    = "tcp"
    cidr_blocks = [var.vpc_cidr]
  }

  # Egress: allow all (k3s needs to pull container images, access registries, and reach upstream APIs)
  #checkov:skip=CKV_AWS_382:k3s host needs broad egress for image pulls, upstream RPCs, and k3s updates
  egress {
    description = "All outbound"
    from_port   = 0
    to_port     = 0
    protocol    = "-1"
    cidr_blocks = ["0.0.0.0/0"]
  }
}

# --- EC2 Instance (on-demand) ---

resource "aws_instance" "k3s" {
  #checkov:skip=CKV_AWS_88:Public IP needed for k3s API access and SSH
  #checkov:skip=CKV_AWS_126:Detailed monitoring not needed for dev/staging k3s host
  #checkov:skip=CKV_AWS_135:EBS optimization automatic for t3+ instances
  #checkov:skip=CKV2_AWS_41:IAM instance profile not required for k3s host
  count = var.use_spot ? 0 : 1

  ami                    = data.aws_ami.ubuntu.id
  instance_type          = var.instance_type
  subnet_id              = var.subnet_id
  vpc_security_group_ids = concat([aws_security_group.k3s.id], var.additional_security_group_ids)
  key_name               = aws_key_pair.k3s.key_name

  associate_public_ip_address = true

  metadata_options {
    http_endpoint = "enabled"
    http_tokens   = "required"
  }

  root_block_device {
    volume_size           = 30
    volume_type           = "gp3"
    encrypted             = true
    delete_on_termination = true
  }

  tags = merge(var.tags, {
    Name = "${var.project_name}-k3s"
  })
}

# --- EC2 Spot Instance (for interruptible workloads like backfill) ---

resource "aws_spot_instance_request" "k3s" {
  #checkov:skip=CKV_AWS_88:Public IP needed for k3s API access and SSH
  #checkov:skip=CKV_AWS_126:Detailed monitoring not needed for dev/staging k3s host
  #checkov:skip=CKV_AWS_135:EBS optimization automatic for t3+ instances
  #checkov:skip=CKV2_AWS_41:IAM instance profile not required for k3s host
  count = var.use_spot ? 1 : 0

  ami                    = data.aws_ami.ubuntu.id
  instance_type          = var.instance_type
  subnet_id              = var.subnet_id
  vpc_security_group_ids = concat([aws_security_group.k3s.id], var.additional_security_group_ids)
  key_name               = aws_key_pair.k3s.key_name

  associate_public_ip_address = true
  wait_for_fulfillment        = true
  spot_type                   = "one-time"

  metadata_options {
    http_endpoint = "enabled"
    http_tokens   = "required"
  }

  root_block_device {
    volume_size           = 30
    volume_type           = "gp3"
    encrypted             = true
    delete_on_termination = true
  }

  tags = merge(var.tags, {
    Name = "${var.project_name}-k3s-spot"
  })
}

# Spot request tags don't propagate to the actual EC2 instance.
# Use aws_ec2_tag to apply Name tag to the fulfilled spot instance.
resource "aws_ec2_tag" "spot_name" {
  count       = var.use_spot ? 1 : 0
  resource_id = aws_spot_instance_request.k3s[0].spot_instance_id
  key         = "Name"
  value       = "${var.project_name}-k3s"
}

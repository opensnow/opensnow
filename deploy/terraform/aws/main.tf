# =============================================================================
# OpenSnow Terraform Module — AWS Infrastructure
#
# Provisions:
#   1. VPC with public + private subnets across multiple AZs
#   2. EKS cluster with two managed node groups:
#        - system:  Cloud Services node, Polaris, PostgreSQL
#        - workers: Warehouse Worker pods (auto-scales to zero when idle)
#   3. S3 bucket for data files (Parquet micro-partitions)
#   4. IAM role for IRSA (IAM Roles for Service Accounts) — grants the
#      OpenSnow service account read/write access to the S3 bucket without
#      long-lived credentials in the cluster
#   5. OpenSnow Helm release deployed into the cluster
#
# Usage:
#   cp terraform.tfvars.example terraform.tfvars
#   # edit terraform.tfvars
#   terraform init && terraform apply
# =============================================================================

# -----------------------------------------------------------------------------
# Local values
# -----------------------------------------------------------------------------
locals {
  # Resolve AZs: use the provided list or default to the first two in the region
  azs = length(var.availability_zones) > 0 ? var.availability_zones : slice(data.aws_availability_zones.available.names, 0, 2)

  # CIDR blocks for subnets — carved out of the VPC CIDR
  public_subnet_cidrs  = [for i, az in local.azs : cidrsubnet(var.vpc_cidr, 8, i)]
  private_subnet_cidrs = [for i, az in local.azs : cidrsubnet(var.vpc_cidr, 8, i + 10)]

  eks_cluster_name = var.cluster_name
}

# -----------------------------------------------------------------------------
# Data sources
# -----------------------------------------------------------------------------
data "aws_availability_zones" "available" {
  state = "available"
}

data "aws_caller_identity" "current" {}

# -----------------------------------------------------------------------------
# VPC
# TODO: Use the terraform-aws-modules/vpc/aws module for production-grade
#       VPC setup (NAT gateways, flow logs, etc.). Stubbed here for clarity.
# -----------------------------------------------------------------------------

# TODO: uncomment and configure when implementing
# module "vpc" {
#   source  = "terraform-aws-modules/vpc/aws"
#   version = "~> 5.0"
#
#   name = "${var.cluster_name}-vpc"
#   cidr = var.vpc_cidr
#
#   azs             = local.azs
#   public_subnets  = local.public_subnet_cidrs
#   private_subnets = local.private_subnet_cidrs
#
#   enable_nat_gateway   = true
#   single_nat_gateway   = true  # set false for HA in production
#   enable_dns_hostnames = true
#
#   # Tags required by EKS for subnet discovery
#   public_subnet_tags = {
#     "kubernetes.io/role/elb"                    = "1"
#     "kubernetes.io/cluster/${local.eks_cluster_name}" = "shared"
#   }
#   private_subnet_tags = {
#     "kubernetes.io/role/internal-elb"           = "1"
#     "kubernetes.io/cluster/${local.eks_cluster_name}" = "shared"
#   }
# }

# -----------------------------------------------------------------------------
# EKS Cluster
# TODO: Use the terraform-aws-modules/eks/aws module.
# -----------------------------------------------------------------------------

# TODO: uncomment and configure when implementing
# module "eks" {
#   source  = "terraform-aws-modules/eks/aws"
#   version = "~> 20.0"
#
#   cluster_name    = local.eks_cluster_name
#   cluster_version = var.kubernetes_version
#
#   vpc_id     = module.vpc.vpc_id
#   subnet_ids = module.vpc.private_subnets
#
#   # Allow kubectl access from the Terraform caller's identity
#   enable_cluster_creator_admin_permissions = true
#
#   eks_managed_node_groups = {
#     system = {
#       instance_types = [var.system_node_instance_type]
#       min_size       = var.system_node_min_count
#       max_size       = var.system_node_max_count
#       desired_size   = var.system_node_desired_count
#       labels = { role = "system" }
#     }
#     workers = {
#       instance_types = [var.worker_node_instance_type]
#       min_size       = var.worker_node_min_count
#       max_size       = var.worker_node_max_count
#       desired_size   = var.worker_node_desired_count
#       labels = { role = "warehouse-worker" }
#       taints = [{ key = "opensnow/worker", effect = "NO_SCHEDULE" }]
#     }
#   }
# }

# -----------------------------------------------------------------------------
# S3 Bucket — data files
# -----------------------------------------------------------------------------

# TODO: uncomment and configure when implementing
# resource "aws_s3_bucket" "data" {
#   bucket        = var.s3_bucket_name
#   force_destroy = var.s3_force_destroy
# }
#
# resource "aws_s3_bucket_versioning" "data" {
#   bucket = aws_s3_bucket.data.id
#   versioning_configuration { status = "Disabled" }
#   # Iceberg manages its own versioning via snapshots; S3 versioning is not needed.
# }
#
# resource "aws_s3_bucket_server_side_encryption_configuration" "data" {
#   bucket = aws_s3_bucket.data.id
#   rule {
#     apply_server_side_encryption_by_default {
#       sse_algorithm = "AES256"
#     }
#   }
# }
#
# resource "aws_s3_bucket_public_access_block" "data" {
#   bucket                  = aws_s3_bucket.data.id
#   block_public_acls       = true
#   block_public_policy     = true
#   ignore_public_acls      = true
#   restrict_public_buckets = true
# }

# -----------------------------------------------------------------------------
# IAM — IRSA role for the OpenSnow service account
# Grants the opensnow-server and opensnow-worker pods S3 read/write access
# without storing static AWS credentials in the cluster.
# -----------------------------------------------------------------------------

# TODO: uncomment and configure when implementing
# module "opensnow_irsa" {
#   source  = "terraform-aws-modules/iam/aws//modules/iam-role-for-service-accounts-eks"
#   version = "~> 5.0"
#
#   role_name = "${var.cluster_name}-opensnow-s3"
#
#   oidc_providers = {
#     main = {
#       provider_arn               = module.eks.oidc_provider_arn
#       namespace_service_accounts = ["${var.opensnow_namespace}:opensnow"]
#     }
#   }
#
#   role_policy_arns = {
#     s3 = aws_iam_policy.opensnow_s3.arn
#   }
# }
#
# resource "aws_iam_policy" "opensnow_s3" {
#   name        = "${var.cluster_name}-opensnow-s3"
#   description = "Allow OpenSnow pods to read/write the data S3 bucket."
#   policy = jsonencode({
#     Version = "2012-10-17"
#     Statement = [
#       {
#         Effect   = "Allow"
#         Action   = ["s3:GetObject", "s3:PutObject", "s3:DeleteObject", "s3:ListBucket"]
#         Resource = [
#           aws_s3_bucket.data.arn,
#           "${aws_s3_bucket.data.arn}/*"
#         ]
#       }
#     ]
#   })
# }

# -----------------------------------------------------------------------------
# Helm release — OpenSnow
# Deployed after EKS and S3 are ready.
# -----------------------------------------------------------------------------

# TODO: uncomment and configure when implementing
# resource "helm_release" "opensnow" {
#   name             = "opensnow"
#   repository       = "https://charts.opensnow.io"
#   chart            = "opensnow"
#   version          = var.opensnow_chart_version
#   namespace        = var.opensnow_namespace
#   create_namespace = true
#
#   set { name = "storage.s3.bucket"; value = var.s3_bucket_name }
#   set { name = "storage.s3.region"; value = var.aws_region }
#   set { name = "storage.iamRoleArn"; value = module.opensnow_irsa.iam_role_arn }
#
#   set_sensitive { name = "auth.adminPassword";    value = var.opensnow_admin_password }
#   set_sensitive { name = "polaris.adminSecret";   value = var.polaris_admin_secret }
#
#   set {
#     name  = "serviceAccount.annotations.eks\\.amazonaws\\.com/role-arn"
#     value = module.opensnow_irsa.iam_role_arn
#   }
#
#   depends_on = [module.eks]
# }

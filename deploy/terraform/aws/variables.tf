# =============================================================================
# OpenSnow Terraform Module — Input Variables (AWS)
# =============================================================================

variable "aws_region" {
  description = "AWS region to deploy OpenSnow into."
  type        = string
  default     = "us-east-1"
}

variable "environment" {
  description = "Deployment environment name (e.g. production, staging). Used in resource tags and naming."
  type        = string
  default     = "production"
}

variable "cluster_name" {
  description = "Name of the EKS cluster. Must be unique within the AWS account and region."
  type        = string
  default     = "opensnow"
}

variable "kubernetes_version" {
  description = "Kubernetes version for the EKS cluster."
  type        = string
  default     = "1.29"
}

# -----------------------------------------------------------------------------
# Networking
# -----------------------------------------------------------------------------

variable "vpc_cidr" {
  description = "CIDR block for the VPC created for the EKS cluster."
  type        = string
  default     = "10.0.0.0/16"
}

variable "availability_zones" {
  description = "List of availability zones to deploy subnets into. Defaults to the first two AZs in the region."
  type        = list(string)
  default     = []
}

# -----------------------------------------------------------------------------
# EKS Node Groups
# -----------------------------------------------------------------------------

variable "system_node_instance_type" {
  description = "EC2 instance type for the system node group (runs Polaris, Postgres, Cloud Services)."
  type        = string
  default     = "m6i.xlarge"
}

variable "system_node_desired_count" {
  description = "Desired number of nodes in the system node group."
  type        = number
  default     = 2
}

variable "system_node_min_count" {
  description = "Minimum number of nodes in the system node group."
  type        = number
  default     = 1
}

variable "system_node_max_count" {
  description = "Maximum number of nodes in the system node group."
  type        = number
  default     = 4
}

variable "worker_node_instance_type" {
  description = "EC2 instance type for warehouse worker nodes (runs DataFusion compute)."
  type        = string
  default     = "r6i.2xlarge"
}

variable "worker_node_desired_count" {
  description = "Desired number of warehouse worker nodes."
  type        = number
  default     = 2
}

variable "worker_node_min_count" {
  description = "Minimum number of warehouse worker nodes."
  type        = number
  default     = 0
}

variable "worker_node_max_count" {
  description = "Maximum number of warehouse worker nodes (for auto-scaling)."
  type        = number
  default     = 20
}

# -----------------------------------------------------------------------------
# S3 Storage
# -----------------------------------------------------------------------------

variable "s3_bucket_name" {
  description = "Name of the S3 bucket for OpenSnow data files. Must be globally unique."
  type        = string
  # No default — must be explicitly set.
}

variable "s3_force_destroy" {
  description = "Allow destroying the S3 bucket even if it contains objects. Set to false in production."
  type        = bool
  default     = false
}

# -----------------------------------------------------------------------------
# OpenSnow Helm release
# -----------------------------------------------------------------------------

variable "opensnow_chart_version" {
  description = "Version of the OpenSnow Helm chart to install."
  type        = string
  default     = "0.1.0"
}

variable "opensnow_namespace" {
  description = "Kubernetes namespace to deploy OpenSnow into."
  type        = string
  default     = "opensnow"
}

variable "opensnow_admin_password" {
  description = "Password for the built-in OpenSnow admin user. Store in Terraform Cloud / AWS Secrets Manager, not in tfvars."
  type        = string
  sensitive   = true
}

variable "polaris_admin_secret" {
  description = "Bootstrap secret for the Apache Polaris root principal."
  type        = string
  sensitive   = true
}

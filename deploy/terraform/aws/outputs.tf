# =============================================================================
# OpenSnow Terraform Module — Outputs
# =============================================================================

# TODO: uncomment each output as the corresponding resource is implemented in main.tf

# output "eks_cluster_name" {
#   description = "Name of the EKS cluster."
#   value       = module.eks.cluster_name
# }

# output "eks_cluster_endpoint" {
#   description = "API server endpoint for the EKS cluster."
#   value       = module.eks.cluster_endpoint
# }

# output "eks_cluster_certificate_authority" {
#   description = "Base64-encoded certificate authority data for the EKS cluster."
#   value       = module.eks.cluster_certificate_authority_data
#   sensitive   = true
# }

# output "s3_bucket_name" {
#   description = "Name of the S3 bucket storing OpenSnow data files."
#   value       = aws_s3_bucket.data.id
# }

# output "s3_bucket_arn" {
#   description = "ARN of the S3 data bucket."
#   value       = aws_s3_bucket.data.arn
# }

# output "opensnow_irsa_role_arn" {
#   description = "IAM role ARN granted to the OpenSnow service account for S3 access (IRSA)."
#   value       = module.opensnow_irsa.iam_role_arn
# }

# output "kubeconfig_command" {
#   description = "AWS CLI command to update your local kubeconfig for this cluster."
#   value       = "aws eks update-kubeconfig --region ${var.aws_region} --name ${module.eks.cluster_name}"
# }

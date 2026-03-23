terraform {
  required_version = ">= 1.6.0"

  required_providers {
    aws = {
      source  = "hashicorp/aws"
      version = "~> 5.0"
    }
    kubernetes = {
      source  = "hashicorp/kubernetes"
      version = "~> 2.0"
    }
    helm = {
      source  = "hashicorp/helm"
      version = "~> 2.0"
    }
  }

  # Recommended: configure a remote backend for state in production.
  # Example (S3 + DynamoDB locking):
  #
  # backend "s3" {
  #   bucket         = "my-terraform-state"
  #   key            = "opensnow/terraform.tfstate"
  #   region         = "us-east-1"
  #   dynamodb_table = "terraform-locks"
  #   encrypt        = true
  # }
}

provider "aws" {
  region = var.aws_region

  default_tags {
    tags = {
      Project     = "opensnow"
      ManagedBy   = "terraform"
      Environment = var.environment
    }
  }
}

# Kubernetes and Helm providers are configured after EKS is created.
# They are wired to the EKS cluster via data sources in main.tf.

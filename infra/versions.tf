terraform {
  required_version = ">= 1.6"
  required_providers {
    github = {
      source  = "integrations/github"
      version = "~> 6.0"
    }
    incus = {
      source  = "lxc/incus"
      version = "~> 1.0"
    }
  }
}

# Authenticates as the repository owner through GITHUB_TOKEN (e.g. `gh auth token`).
provider "github" {
  owner = var.owner
}

# Talks to the local Incus daemon.
provider "incus" {}

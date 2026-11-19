variable "owner" {
  description = "GitHub account that owns the repository."
  type        = string
  default     = "vrinek"
}

variable "repository" {
  type    = string
  default = "speedrun-evals"
}

variable "manager_username" {
  description = "GitHub login of the machine account the manager agent acts as. Create it by hand first."
  type        = string
}

variable "manager_token" {
  description = "Classic personal access token of the manager account, scopes: public_repo, notifications."
  type        = string
  sensitive   = true
}

variable "model_endpoint" {
  description = "OpenAI-compatible base URL of the local model, as reachable from inside the VM."
  type        = string
}

variable "model_id" {
  type    = string
  default = "qwen38-flash-next"
}

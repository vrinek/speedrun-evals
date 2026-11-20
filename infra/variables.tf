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
  description = "GitHub login of the machine account the manager agent acts as. Leave empty until the account exists."
  type        = string
  default     = ""
}

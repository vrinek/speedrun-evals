# The manager's whole world: a VM holding its GitHub token, a checkout of the
# repository and a route to the local model. Nothing else of the host is visible.
resource "incus_instance" "manager" {
  name  = "speedrun-manager"
  type  = "virtual-machine"
  image = "images:debian/13/cloud"

  config = {
    "limits.cpu"    = "2"
    "limits.memory" = "4GiB"
    "cloud-init.user-data" = templatefile("${path.module}/manager/cloud-init.yaml", {
      repo           = "${var.owner}/${var.repository}"
      manager_token  = var.manager_token
      model_endpoint = var.model_endpoint
      model_id       = var.model_id
      poll_script    = indent(6, file("${path.module}/manager/poll.sh"))
      prompt         = indent(6, file("${path.module}/manager/PROMPT.md"))
    })
  }
}

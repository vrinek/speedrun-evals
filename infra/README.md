# Infrastructure

One Terraform root that sets up:

1. The GitHub repository: settings, labels, a ruleset that makes `main`
   pull-request-only, and triage access for the manager's account.
2. A local Incus VM in which the manager agent (a local model driven by the
   [Pi](https://github.com/earendil-works/pi) harness) polls GitHub
   notifications every 15 minutes and handles each one.

## What the manager can and cannot do

| | |
|---|---|
| Can | Comment, label, close and reopen issues and pull requests; open pull requests from its own fork |
| Cannot | Push to the repository, merge, approve, change settings, read anything on the host |

Its instructions are [`manager/PROMPT.md`](manager/PROMPT.md) plus the repository's `AGENTS.md`.

## Before applying

- Create the manager's GitHub account by hand, and a classic token for it with
  scopes `public_repo` and `notifications`. (The notifications API does not
  accept fine-grained tokens.)
- Make the model reachable from the VM. The model listens on the host's
  loopback only, and a VM cannot reach that. Expose it on the Incus bridge
  address only, for example with a `socat` service on the host, and pass that
  URL as `model_endpoint`.

```sh
cd infra
terraform init
GITHUB_TOKEN=$(gh auth token) terraform apply \
  -var manager_username=<login> -var model_endpoint=http://<bridge-ip>:8126/v1
# Terraform prompts for manager_token.
```

The token ends up in Terraform state and in the VM's cloud-init data. Keep
`terraform.tfstate` out of version control (it is git-ignored).

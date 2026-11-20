# Infrastructure

Two parts.

## 1. GitHub (Terraform)

The repository's settings, labels and a ruleset that makes `main`
pull-request-only with green CI.

```sh
cd infra
terraform init
GITHUB_TOKEN=$(gh auth token) terraform apply
```

`terraform.tfstate` is git-ignored and holds no secrets.

## 2. The manager (podman + systemd)

A local model, driven by the [Pi](https://github.com/earendil-works/pi)
harness in a rootless podman container. A systemd user timer starts it every 15
minutes; it handles each unread GitHub notification and exits.

| | |
|---|---|
| Can | Comment on issues and pull requests; open pull requests from its own fork |
| Cannot | Label, close, push to the repository, merge, change settings. Its account has no access to the repository beyond any GitHub user's. |
| Sees of the host | The model's unix socket and its own home volume. No host files, no host ports. |

Its instructions are [`manager/PROMPT.md`](manager/PROMPT.md) plus the repository's `AGENTS.md`.

Set-up, once:

1. Create the manager's GitHub account by hand, and a classic token for it with
   scopes `public_repo` and `notifications`. (The notifications API does not
   accept fine-grained tokens.)
2. Write the token where only you can read it:

   ```sh
   install -m 600 /dev/null ~/.config/speedrun-manager.env
   printf 'GH_TOKEN=<token>\nREPO=vrinek/speedrun-evals\n' > ~/.config/speedrun-manager.env
   ```
3. Build and enable:

   ```sh
   podman build -t speedrun-manager infra/manager
   cp infra/manager/speedrun-manager.{service,timer} ~/.config/systemd/user/
   systemctl --user daemon-reload
   systemctl --user enable --now speedrun-manager.timer
   ```

Watch it with `journalctl --user -u speedrun-manager -f`. Stop it with
`systemctl --user disable --now speedrun-manager.timer`.

The model is expected as a unix socket at `$XDG_RUNTIME_DIR/flashnext.sock`
speaking the OpenAI API, with model id `qwen38-flash-next`. To use another
model, change `manager/pi/models.json` and the socket path in the service.

resource "github_repository" "this" {
  name         = var.repository
  description  = "How fast and how well local LLMs finish benchmarks on your hardware"
  visibility   = "public"
  has_issues   = true
  has_wiki     = false
  has_projects = false

  allow_merge_commit     = false
  allow_rebase_merge     = false
  allow_squash_merge     = true
  delete_branch_on_merge = true
}

# The website, published by .github/workflows/ci.yml.
resource "github_repository_pages" "this" {
  repository = github_repository.this.name
  build_type = "workflow"
}

# The labels AGENTS.md tells the manager to triage with.
resource "github_issue_labels" "this" {
  repository = github_repository.this.name

  dynamic "label" {
    for_each = {
      bug       = "d73a4a"
      benchmark = "5319e7"
      model     = "0e8a16"
      website   = "1d76db"
      question  = "d876e3"
      result    = "bfdadc"
      droid     = "000000" # starts .github/workflows/droid.yml, owner only
    }
    content {
      name  = label.key
      color = label.value
    }
  }
}

# Nobody pushes to main: every change is a pull request with green CI and one
# approval. The manager's approval does not count, because triage access cannot
# approve, so a human always decides what is merged.
resource "github_repository_ruleset" "main" {
  name        = "main"
  repository  = github_repository.this.name
  target      = "branch"
  enforcement = "active"

  conditions {
    ref_name {
      include = ["~DEFAULT_BRANCH"]
      exclude = []
    }
  }

  rules {
    deletion         = true
    non_fast_forward = true

    pull_request {
      required_approving_review_count = 1
    }

    required_status_checks {
      required_check {
        context = "check"
      }
    }
  }

  # The owner may bypass, otherwise a one-person project cannot merge its own work.
  bypass_actors {
    actor_id    = 5 # the built-in "Repository admin" role
    actor_type  = "RepositoryRole"
    bypass_mode = "always"
  }
}

# Triage: label, comment on, close and reopen issues and pull requests.
# No push access. The manager proposes code changes from its own fork.
resource "github_repository_collaborator" "manager" {
  repository = github_repository.this.name
  username   = var.manager_username
  permission = "triage"
}

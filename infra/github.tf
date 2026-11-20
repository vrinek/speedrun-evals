resource "github_repository" "this" {
  name         = var.repository
  description  = "How fast and how well local LLMs finish benchmarks on your hardware"
  homepage_url = "https://${var.owner}.github.io/${var.repository}/"
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

# Nobody pushes to main: every change is a pull request with green CI.
# No approval is required. Only the owner has write access, so merging is
# already the owner's decision, and GitHub does not let anyone approve a pull
# request opened under their own account.
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
      required_approving_review_count = 0
    }

    required_status_checks {
      required_check {
        context = "check"
      }
    }
  }
}

# The manager's account gets no access to this repository at all. GitHub only
# offers the triage role in organisations; on a personal repository the one
# choice is full write access, which would let the manager merge. As an
# ordinary GitHub user it can still comment, review and open pull requests
# from its fork, which is all it needs.

#!/usr/bin/env bash
# Hands each unread GitHub notification to the agent, one at a time.
# Notifications never include the manager's own actions, so it cannot answer itself.
# ponytail: shell, because it is forty lines of glue around `gh` and `pi`.
set -euo pipefail
cd "$HOME"

[ -d repo ] || gh repo clone "$REPO" repo
git -C repo pull --quiet --ff-only
# Watch the repository so every new issue and pull request produces a notification.
gh api -X PUT "repos/$REPO/subscription" -F subscribed=true --silent

gh api notifications --paginate --jq ".[] | select(.repository.full_name == \"$REPO\") | [.id, .subject.type, .subject.url] | @tsv" |
  while IFS=$'\t' read -r id type url; do
    echo "Handling $type $url"
    # A failed run leaves the notification unread, so the next poll retries it.
    if (cd repo && pi --print --no-session --append-system-prompt "$HOME/PROMPT.md" \
      "A GitHub notification arrived for a $type. Its API URL is $url. Read it with \`gh api\`, then handle it." </dev/null); then
      gh api -X PATCH "notifications/threads/$id" --silent
    fi
  done

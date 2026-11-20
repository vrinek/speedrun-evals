#!/usr/bin/env bash
# Hands each unread GitHub notification to the agent, one at a time, then exits.
# Notifications never include the manager's own actions, so it cannot answer itself.
# ponytail: shell, because it is forty lines of glue around `gh` and `pi`.
set -euo pipefail

# The container cannot reach the host's loopback. The model arrives as a unix
# socket instead; splice it onto the port Pi is configured for.
socat TCP-LISTEN:8126,bind=127.0.0.1,fork,reuseaddr UNIX-CONNECT:/run/model.sock &
trap 'kill %1' EXIT

cd "$HOME"
git config --global user.name "Speedrun Evals manager"
git config --global user.email "manager@speedrun-evals.invalid"
[ -d repo ] || gh repo clone "$REPO" repo
git -C repo pull --quiet --ff-only
# Watch the repository so every new issue and pull request produces a notification.
gh api -X PUT "repos/$REPO/subscription" -F subscribed=true --silent

gh api notifications --paginate --jq ".[] | select(.repository.full_name == \"$REPO\") | [.id, .subject.type, .subject.url] | @tsv" |
  while IFS=$'\t' read -r id type url; do
    echo "Handling $type $url"
    # A failed run leaves the notification unread, so the next poll retries it.
    # The 30-minute timeout matches Pi's: the model may be busy with someone else.
    if (cd repo && pi --print --no-session --append-system-prompt /manager/PROMPT.md \
      "A GitHub notification arrived for a $type. Its API URL is $url. Read it with \`gh api\`, then handle it." </dev/null); then
      gh api -X PATCH "notifications/threads/$id" --silent
    fi
  done

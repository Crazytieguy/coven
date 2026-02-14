#!/bin/bash
# PostToolUse hook: inject new issue/PR comments into Claude's context.
# Requires env vars: ISSUE_NUMBER
# GITHUB_REPOSITORY is set automatically by GitHub Actions
# State file: /tmp/claude-last-comment-id (initialized by workflow step)

STATE_FILE="/tmp/claude-last-comment-id"

# Rate limit: skip if checked within last 10 seconds
if [ -f "$STATE_FILE" ]; then
  last_check=$(stat -c %Y "$STATE_FILE" 2>/dev/null)
  now=$(date +%s)
  if [ $(( now - last_check )) -lt 10 ]; then
    exit 0
  fi
fi

# Single API call
comments=$(gh api "repos/$GITHUB_REPOSITORY/issues/$ISSUE_NUMBER/comments" 2>/dev/null) || exit 0

last_seen=$(cat "$STATE_FILE" 2>/dev/null || echo "0")
last_seen=${last_seen:-0}

# Always update state (updates mtime for rate limiting + saves latest ID)
echo "$comments" | jq -r 'if length > 0 then last.id else 0 end' > "$STATE_FILE"

# Filter new comments from humans only
new_comments=$(echo "$comments" | jq -r --argjson last_seen "$last_seen" \
  '[.[] | select(.id > $last_seen and .user.login != "claude[bot]")]
   | map("@" + .user.login + ": " + .body)
   | join("\n\n")')

if [ -z "$new_comments" ]; then
  exit 0
fi

# Try structured output first; if Claude doesn't support it, plain stdout also works
jq -n --arg ctx "New comment posted on the issue while you're working:\n\n$new_comments" \
  '{"hookSpecificOutput": {"additionalContext": $ctx}}'

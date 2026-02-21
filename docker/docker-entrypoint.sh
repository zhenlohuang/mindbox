#!/usr/bin/env bash

set -euo pipefail

# Claude Code expects ANTHROPIC_API_KEY; allow legacy ANTHROPIC_AUTH_TOKEN as fallback.
if [[ -z "${ANTHROPIC_API_KEY:-}" && -n "${ANTHROPIC_AUTH_TOKEN:-}" ]]; then
  export ANTHROPIC_API_KEY="${ANTHROPIC_AUTH_TOKEN}"
fi

SKILLS_DIR="/mindbox/skills"
if [ -d "$SKILLS_DIR" ]; then
  for skill in "$SKILLS_DIR"/*/; do
    [ -d "$skill" ] && claude skills add "$skill" || true
  done
fi

exec /usr/bin/supervisord -n -c /etc/supervisor/conf.d/mindbox.conf

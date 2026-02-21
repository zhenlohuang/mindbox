#!/usr/bin/env bash

set -euo pipefail

# Claude Code expects ANTHROPIC_API_KEY; allow legacy ANTHROPIC_AUTH_TOKEN as fallback.
if [[ -z "${ANTHROPIC_API_KEY:-}" && -n "${ANTHROPIC_AUTH_TOKEN:-}" ]]; then
  export ANTHROPIC_API_KEY="${ANTHROPIC_AUTH_TOKEN}"
fi

exec /usr/bin/supervisord -n -c /etc/supervisor/conf.d/mindbox.conf

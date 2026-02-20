#!/usr/bin/env bash

set -euo pipefail

exec /usr/bin/supervisord -n -c /etc/supervisor/conf.d/mindbox.conf

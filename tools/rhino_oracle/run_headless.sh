#!/usr/bin/env bash
set -euo pipefail

if (( $# < 2 )); then
    echo "usage: $0 {compare|rhino|viboceros} REQUEST [oracle options...]" >&2
    exit 2
fi

for dependency in xvfb-run Xvfb i3 python3; do
    if ! command -v "$dependency" >/dev/null 2>&1; then
        echo "headless Rhino oracle requires '$dependency'" >&2
        exit 2
    fi
done

ORACLE_REPO_ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
ORACLE_I3_CONFIG="$ORACLE_REPO_ROOT/tools/rhino_oracle/i3-headless.conf"
ORACLE_I3_LOG="${TMPDIR:-/tmp}/viboceros-rhino-oracle-i3.log"
export ORACLE_I3_CONFIG ORACLE_I3_LOG
cd "$ORACLE_REPO_ROOT"

exec xvfb-run -a -s '-screen 0 1920x1080x24' bash -c '
    env -u I3SOCK i3 -a -c "$ORACLE_I3_CONFIG" >"$ORACLE_I3_LOG" 2>&1 &
    exec python3 -m tools.rhino_oracle "$@"
' viboceros-rhino-oracle "$@"

#!/usr/bin/env bash
# End-to-end check of survival building: start a server, run the
# `survival_build_check` client example against it, restart the server
# gracefully (which saves terrain persistence) and verify that the placed
# block survived.
#
# Build both binaries first:
#
#   cargo build --release --bin veloren-server-cli
#   cargo build --release -p veloren-client --features bin_bot \
#       --example survival_build_check
#
# The userdata directory needs a server config with `auth_server_address:
# None`, `experimental_terrain_persistence: true` and the TCP port below, and a
# server-cli config with `shutdown_signals: [SIGUSR1, ...]` and
# `update_shutdown_grace_period_secs: 0`, so that the restart is quick and
# saves the terrain. Unix only, as the restart uses SIGUSR1.
#
# Usage: E2E_USERDATA=/path/to/userdata [E2E_PORT=14104] scripts/survival_build_e2e.sh
set -u
WT=$(cd "$(dirname "$0")/.." && pwd)
DATA=${E2E_USERDATA:?set E2E_USERDATA to the userdata directory of the test server}
PORT=${E2E_PORT:-14104}
SERVER=$WT/target/release/veloren-server-cli
CHECK=$WT/target/release/examples/survival_build_check
STATE=$DATA/state.txt
export VELOREN_USERDATA=$DATA VELOREN_ASSETS=$WT/assets
export RUST_LOG=${RUST_LOG:-info,veloren_server::events::building=debug}

start_server() {
    nice -n 5 "$SERVER" --no-auth --non-interactive >> "$DATA/server-$1.log" 2>&1 &
    SERVER_PID=$!
    for _ in $(seq 1 600); do
        ss -ltn | grep -q ":$PORT " && return 0
        kill -0 $SERVER_PID 2>/dev/null || { echo "server exited early"; return 1; }
        sleep 1
    done
    echo "server did not start listening"; return 1
}

stop_server() {
    kill -USR1 $SERVER_PID
    for _ in $(seq 1 120); do
        kill -0 $SERVER_PID 2>/dev/null || return 0
        sleep 1
    done
    echo "server did not stop, killing"; kill -9 $SERVER_PID
}

"$SERVER" admin add buildcheck admin > "$DATA/admin.log" 2>&1
rm -f "$STATE"

start_server run || exit 1
echo "== run"; "$CHECK" run $PORT "$STATE"; RUN=$?
stop_server
[ -f "$STATE" ] || { echo "no state file, skipping verify"; exit 1; }

start_server verify || exit 1
echo "== verify"; "$CHECK" verify $PORT "$STATE"; VERIFY=$?
stop_server

echo "run exit $RUN, verify exit $VERIFY"
[ $RUN -eq 0 ] && [ $VERIFY -eq 0 ]

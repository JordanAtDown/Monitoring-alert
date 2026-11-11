#!/usr/bin/env bash
# Runs the dashboard locally, on a COPY of a database, with a fake LibreHardwareMonitor.
# Nothing of yours is touched: the original database is only copied.
#
#   tools/demo.sh                          synthetic data (14 months, several disks, a slow drift)
#   tools/demo.sh /path/to/temperatures.db your own data (e.g. /mnt/e/monitoring.db/temperatures.db)
#   tools/demo.sh --update [database]      also open a second address that pretends an update exists
#
# Port: DEMO_PORT (default 18797). Ctrl+C stops everything and deletes the copy.
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PORT="${DEMO_PORT:-18797}"
LHM_PORT=$((PORT + 100))
UPDATE=0
SOURCE=""
for arg in "$@"; do
    case "$arg" in
        --update) UPDATE=1 ;;
        *) SOURCE="$arg" ;;
    esac
done

echo "==> Building the dashboard and the program"
(cd "$ROOT/frontend" && { [ -d node_modules ] || npm ci --silent; } && npm run build --silent)
(cd "$ROOT/backend" && cargo build --quiet)

DIR="$(mktemp -d /tmp/monitoring-demo.XXXXXX)"
PIDS=()
cleanup() {
    for pid in "${PIDS[@]:-}"; do kill "$pid" 2>/dev/null || true; done
    rm -rf "$DIR"
    echo "Stopped, copy deleted."
}
trap cleanup EXIT INT TERM

if [ -n "$SOURCE" ]; then
    echo "==> Copying $SOURCE (the original is not modified)"
    cp "$SOURCE" "$DIR/temperatures.db"
    [ -f "$SOURCE-wal" ] && cp "$SOURCE-wal" "$DIR/temperatures.db-wal"
else
    echo "==> Creating synthetic data"
    python3 "$ROOT/tools/demo_seed.py" "$DIR/temperatures.db"
fi

printf 'lhm_port = %s\nweb_port = %s\nlog_dir = "."\ncollect_interval_secs = 60\nraw_retention_days = 365\n' "$LHM_PORT" "$PORT" > "$DIR/config.toml"

python3 "$ROOT/tools/demo_lhm.py" "$DIR/temperatures.db" "$LHM_PORT" & PIDS+=($!)
sleep 1
(cd "$DIR" && exec "$ROOT/backend/target/debug/monitoring" --db temperatures.db serve) > "$DIR/serve.log" 2>&1 & PIDS+=($!)

for _ in $(seq 1 40); do
    curl -fs "http://127.0.0.1:$PORT/api/series" > /dev/null 2>&1 && break
    sleep 0.5
done

echo
echo "  Tableau de bord :  http://127.0.0.1:$PORT/"
if [ "$UPDATE" = 1 ]; then
    python3 "$ROOT/tools/demo_update_proxy.py" "$PORT" $((PORT + 1)) & PIDS+=($!)
    echo "  Avec une mise à jour simulée :  http://127.0.0.1:$((PORT + 1))/"
fi
echo
echo "  Pour voir l'état « problème » : arrêtez le faux LibreHardwareMonitor (kill ${PIDS[0]})."
echo "  Ctrl+C pour tout arrêter."
wait "${PIDS[1]}"

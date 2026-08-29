#!/usr/bin/env bash
# Run the browser specs and exit non-zero if any fail — the shape CI wants.
#
# Two things are not optional:
#   * serve mode, because the specs drive a browser at this very server
#   * the private-network opt-in, because that server is on loopback and the
#     renderer refuses private addresses by default
set -uo pipefail
cd "$(dirname "$0")/../.."
PORT="${PORT:-8599}"

# Refuse to share a port. Without this the script cheerfully talks to whatever
# is already listening — a stray CommandBox server answered these requests once
# and reported the specs as "file not found" from a completely different webroot.
if lsof -nP -iTCP:"$PORT" -sTCP:LISTEN >/dev/null 2>&1; then
  echo "port $PORT is already in use — set PORT=<free port> and try again" >&2
  exit 1
fi

OBSCURA_ALLOW_PRIVATE_NETWORK=1 rustcfml --serve --port "$PORT" >/tmp/browser-testbox.log 2>&1 &
server=$!
trap 'kill $server 2>/dev/null || true' EXIT

for _ in $(seq 1 40); do
  curl -sf -m 1 "http://127.0.0.1:$PORT/examples/05-testbox/signup.cfm" >/dev/null && break
  sleep 0.5
done

out=$(curl -s "http://127.0.0.1:$PORT/examples/05-testbox/runner.cfm?reporter=text")
echo "$out"

# Gate on the totals line. Match it exactly — an earlier version looked for
# "Fail: 0" while the reporter writes "Failed: 0", so the gate failed the build
# whatever the tests did. Check the passing path as well as the failing one.
echo "$out" | grep -q "\[Failed: 0\]" || exit 1
echo "$out" | grep -q "\[Errors: 0\]"  || exit 1
echo "$out" | grep -q "\[Passed: 0\]"  && { echo "no specs ran" >&2; exit 1; }
exit 0

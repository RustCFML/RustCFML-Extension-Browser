#!/usr/bin/env bash
# Build, install and run every test.
#
# `rustcfml ext build` is not `cargo build` — a plain cargo build leaves the
# installed .rcx stale, which shows up as a missing method rather than as a
# stale artifact.
#
# Run the whole suite, not one file. A context-lifetime bug once failed four of
# six tests while none of them failed individually.
set -uo pipefail
cd "$(dirname "$0")/.."
# Stop if either step fails: otherwise the suite quietly runs against whatever
# build was installed last, and passes.
version=$(sed -n 's/^version *= *"\(.*\)"/\1/p' Cargo.toml | head -1)
rustcfml ext build . >/dev/null || { echo "ext build failed"; exit 1; }
rustcfml ext install "browser-${version}.rcx" --user >/dev/null || { echo "install of browser-${version}.rcx failed"; exit 1; }

# Fixture server for the interception test. Loopback, so that one test needs the
# private-network opt-in; the rest must NOT have it, or they would stop
# exercising the default egress policy.
python3 tests/mock_server.py >/dev/null 2>&1 &
server=$!
trap 'kill $server 2>/dev/null || true' EXIT
sleep 1

status=0
for t in tests/*.cfm; do
  echo "=== $t"
  case "$(basename "$t")" in
    mock.cfm) OBSCURA_ALLOW_PRIVATE_NETWORK=1 rustcfml "$t" || status=1 ;;
    *)        rustcfml "$t" || status=1 ;;
  esac
done
exit $status

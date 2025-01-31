#! /bin/sh

set -e
set -x

trap 'echo "Killing background jobs..."; kill $(jobs -p)' EXIT

args="$@"

if ! test -d webapp/node_modules; then
    (cd webapp && npm ci --prefer-offline)
fi

export RUST_BACKTRACE=1

(cd webapp && npm start) &
cargo watch -w src -i webapp -x "run ${RELEASE} server -k --no-tls --no-auth ${args}"

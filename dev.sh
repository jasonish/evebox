#! /bin/sh

set -e
set -x

trap 'echo "Killing background jobs..."; kill $(jobs -p)' EXIT

args="$@"

if ! test -d webapp/node_modules; then
    (cd webapp && make install-deps)
fi

(cd webapp && npm start) &
cargo watch -i webapp -x "run server ${args}"

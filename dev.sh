#! /bin/sh

set -e
set -x

trap 'echo "Killing background jobs..."; kill $(jobs -p)' EXIT

args="$@"

(cd webapp && npm start) &
cargo watch -i webapp -x "run server ${args}"

#! /bin/sh

set -x
set -e

trap 'echo "Killing background jobs..."; kill $(jobs -p)' EXIT

args="$@"

(cd webapp && make serve) &

reflex -s -R bindata\.go -r \.go$ -- \
       sh -c "NO_WEBAPP=1 make evebox && \
                 ./evebox server --dev http://localhost:4200 ${args}"

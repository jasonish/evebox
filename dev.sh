#! /bin/sh

set -x
set -e

trap 'echo "Killing background jobs..."; kill $(jobs -p)' EXIT

args="$@"

(cd webapp && make serve) &

reflex -s -R -packr\.go -r \.go$ -- \
       sh -c "rm -f evebox && make evebox && \
                 ./evebox server ${args}"

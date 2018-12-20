#! /bin/sh

set -x
set -e

trap 'echo "Killing background jobs..."; kill $(jobs -p)' EXIT

args="$@"
command=""

case "$1" in
    -*)
	command=server
	;;
esac

(cd webapp && make serve) &

$HOME/go/bin/reflex -s -R -packr\.go -r \.go$ -- \
       sh -c "rm -f evebox && make evebox && \
                 ./evebox -v ${command} ${args}"

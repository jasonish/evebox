#! /bin/sh

set -e

# Open up the bind host to 0.0.0.0 as we are running in a container.
if [ "${EVEBOX_HTTP_HOST}" = "" ]; then
    EVEBOX_HTTP_HOST=0.0.0.0
    export EVEBOX_HTTP_HOST
fi

# Add evebox as command if needed
if [ "${1:0:1}" = "-" ]; then
    set -- evebox server "$@"
fi

exec "$@"

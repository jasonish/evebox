#! /bin/bash

set -e

if [[ "$(whoami)" == "builder" ]]; then
    if [[ "${REAL_UID}" && "${REAL_GID}" ]]; then
	sudo usermod --uid "${REAL_UID}" builder > /dev/null
	sudo groupmod --gid "${REAL_GID}" builder > /dev/null
	sudo chown -R builder.builder /home/builder
    else
	echo "warning: real uid and gid are unknown: permissions may be wrong"
    fi
fi

. /opt/rh/rh-ruby23/enable

export PATH=$HOME/go/bin:$PATH

if [ "$1" ]; then
    exec $@
else
    exec /bin/bash
fi

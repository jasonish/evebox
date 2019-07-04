#! /bin/bash

set -e

if [ "${REAL_UID}" != "0" ]; then
    groupmod --gid "${REAL_GID}" builder
    usermod --uid "${REAL_UID}" builder
fi

export PATH=/home/builder/go/bin:$PATH

if [ -e /opt/osxcross ]; then
    export PATH=/opt/osxcross/target/bin:$PATH
fi

if [ "${REAL_UID}" = "0" ]; then
    if [ "$1" ]; then
	exec /bin/bash -c "$@"
    else
	exec /bin/bash
    fi
else
    export HOME=/home/builder
    if [ "$1" ]; then
	exec su builder -m -c "$@"
    else
	exec su builder
    fi
fi

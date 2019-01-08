#! /bin/bash

set -e

groupmod --gid "${REAL_GID}" builder
usermod --uid "${REAL_UID}" builder

. /opt/rh/rh-ruby23/enable

export PATH=$HOME/go/bin:$PATH

if [ "$1" ]; then
    exec su builder -m -c "HOME=/home/builder $@"
else
    exec su builder
fi

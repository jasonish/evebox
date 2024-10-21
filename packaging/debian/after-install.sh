#! /bin/bash

set -e
set -x

USERNAME=evebox
HOMEDIR=/var/lib/evebox

if ! /usr/bin/getent passwd ${USERNAME} > /dev/null; then
    useradd --system \
            --home-dir ${HOMEDIR} \
            --user-group \
            --shell /usr/sbin/nologin \
            ${USERNAME}
fi

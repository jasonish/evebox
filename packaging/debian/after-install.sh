#! /bin/bash

set -e

USERNAME=evebox
HOMEDIR=/var/lib/evebox

if ! /usr/bin/getent passwd ${USERNAME} > /dev/null; then
    if test -e /usr/sbin/adduser; then
	/usr/sbin/adduser --system --home ${HOMEDIR} --group \
            --disabled-login ${USERNAME}
    else
	echo "warning: adduser not found, evebox user not created"
    fi
fi


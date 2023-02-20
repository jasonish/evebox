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

if ! /bin/systemctl daemon-reload > /dev/null 2>&1; then
    # Exit now if this failed. May be running in a container.
    exit 0
fi

# Restart evebox if running.
if /bin/systemctl status evebox > /dev/null; then
    echo "Restarting evebox."
    /bin/systemctl restart evebox
fi

# Restart evebox-agent if running.
if /bin/systemctl status evebox-agent > /dev/null; then
    echo "Restarting evebox-agent."
    /bin/systemctl restart evebox-agent
fi

#! /bin/bash

set -e

USERNAME=evebox
GROUPNAME=evebox
HOMEDIR=/var/lib/evebox

getent group ${GROUPNAME} >/dev/null || groupadd -r ${GROUPNAME}
getent passwd ${USERNAME} >/dev/null || \
    useradd -r -g ${GROUPNAME} -d ${HOMEDIR} -s /sbin/nologin \
    -c "EveBox Server" ${USERNAME}

/bin/install -o ${USERNAME} -g ${GROUPNAME} -d /var/lib/evebox

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

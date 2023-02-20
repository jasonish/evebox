#! /bin/sh

set -e

USERNAME=evebox
GROUPNAME=evebox
HOMEDIR=/var/lib/evebox

getent group ${GROUPNAME} >/dev/null || groupadd -r ${GROUPNAME}
getent passwd ${USERNAME} >/dev/null || \
    useradd -r -g ${GROUPNAME} -d ${HOMEDIR} -s /sbin/nologin \
    -c "EveBox Server" ${USERNAME}

/bin/install -o ${USERNAME} -g ${GROUPNAME} -d /var/lib/evebox

exit 0

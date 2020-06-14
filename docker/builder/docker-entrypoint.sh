#! /bin/bash

set -e
set -x

if [ -e /opt/rh/rh-ruby23/enable ]; then
    . /opt/rh/rh-ruby23/enable
fi

if [ -e /opt/osxcross ]; then
    export PATH=/opt/osxcross/target/bin:$PATH
fi

$@

if [ "${FIX_PERMS}" = "true" ]; then
    if [ "${REAL_UID}" -a "${REAL_GID}" ]; then
        chown -R "${REAL_UID}:${REAL_GID}" dist target
    fi
fi

#! /bin/bash

set -e
set -x

USERNAME=evebox
HOMEDIR=/var/lib/evebox

adduser --system --home ${HOMEDIR} --group --disabled-login ${USERNAME}

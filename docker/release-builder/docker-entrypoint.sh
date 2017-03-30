#! /bin/sh

. /opt/rh/rh-ruby22/enable

make install-deps
make release rpm deb
cp -a dist/* /dist
chown -R ${REAL_UID}:${REAL_GID} /dist


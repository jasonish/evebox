#! /bin/bash

export PATH=/usr/pgsql-9.6/bin:$PATH
PGDATA=/var/lib/pgsql/9.6/data

# Docker container starts as root so we can fix up permissions, we
# then switch to the postgres user for initialization of postgres and
# running.
if [ "$(id -u)" = "0" ]; then
    mkdir -p ${PGDATA}
    chown -R postgres ${PGDATA}
    exec su postgres ${BASH_SOURCE}
fi

if ! test -e ${PGDATA}/PG_VERSION; then
    initdb --encoding=UTF8 ${PGDATA}
    echo "host all all all md5" >> ${PGDATA}/pg_hba.conf
    pg_ctl -D ${PGDATA} -w start
    createuser evebox
    createdb --owner=evebox evebox
else
    pg_ctl -D ${PGDATA} -w start
fi

if [ "${PGPASSWORD}" = "" ]; then
    echo "WARNING: database will NOT have a password"
else
    psql <<EOF
alter user evebox with superuser password '${PGPASSWORD}';
EOF
fi

pg_ctl -D ${PGDATA} -w -m fast stop

exec postgres -D ${PGDATA} \
     -c log_destination=stderr \
     -c logging_collector=off \
     -c listen_addresses='*' \
     -c constraint_exclusion=on

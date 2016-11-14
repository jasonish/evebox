#! /bin/sh

POSTGRES=postgres
PSQL=psql
INITDB=initdb
PGCTL=pg_ctl
CREATEDB=createdb

PGVERSION=$(${POSTGRES} --version | sed -n 's/[^0-9]*\([0-9]\.[0-9]\).*/\1/p')
PGHOST=127.0.0.1
PGPORT=8432
PGDATA=./data/pgdata${PGVERSION}
PGDATABASE=evebox
PGUSER=evebox

export PGHOST PGPORT PGDATABASE PGUSER

init() {
    initdb --auth-local=trust --auth-host=trust --username=${PGUSER} \
	   --encoding=UTF8 ${PGDATA}
    start
    ${CREATEDB} ${PGDATABASE}
    ${PSQL} -c 'CREATE EXTENSION "uuid-ossp"'
    ${PSQL} -c 'CREATE EXTENSION "pgcrypto"'
    stop
}

start() {
    ${PGCTL} -D ${PGDATA} -l postgres.log -w start -o "-k /tmp"
}

stop() {
    ${PGCTL} -D ${PGDATA} -w -m fast stop
}

case "$1" in

    init)
	init
	;;

    reinit)
	stop
	rm -rf ${PGDATA}
	init
	start
	;;

    start)
	start
	;;

    stop)
	stop
	;;

    restart)
	stop
	start
	;;

    dump)
	pg_dump --data-only
	;;

    restore)
	shift
	pg_restore "$@"
	;;

    env)
	exec "$@"
	;;

    psql)
	shift
	${PSQL} ${PGDATABSE} "$@"
	;;

esac

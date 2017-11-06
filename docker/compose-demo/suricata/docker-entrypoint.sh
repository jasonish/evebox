#! /bin/bash

rotate_logs() {
    while true; do

	sleep 3600

	if ! test -e /var/run/suricata.pid; then
	    echo "Suricata PID file does not exist. Not rotating logs."
	else
	    echo "Cleaning Suricata logs."
	    find /var/log/suricata -type f -delete
	    kill -HUP $(cat /var/run/suricata.pid)
	fi

    done
}

get_interface() {

    if [ "${INTERFACE}" != "" ]; then
	echo "${INTERFACE}"
	return
    fi

    default_if=$(ip -o -4 route show to default | awk '{ print $5 }')

    if [ "$default_if" != "" ]; then
	echo "INTERFACE environment variable not set. Will try ${default_if}." > /dev/stderr
	echo "${default_if}"
	return
    fi

    echo "WARNING: INTERFACE environment variable net set, will try eth0" > /dev/stderr
    echo "eth0"

}

rotate_logs &

interface=$(get_interface)

exec /usr/sbin/suricata -c /etc/suricata/suricata-demo.yaml \
     -i "${interface}" \
     --pidfile /var/run/suricata.pid

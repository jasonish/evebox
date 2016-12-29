#! /bin/sh

set -e

# This is a development hack.
if [ -e /evebox/backend/evebox ]; then
    echo "Replacing /usr/local/bin/evebox."
    cp /evebox/backend/evebox /usr/local/bin/evebox
fi

# Add evebox as command if needed
if [ "${1:0:1}" = "-" ]; then
    set -- evebox server "$@"
fi

if [ "$1" = "evebox" ]; then
    if [ "$ELASTICSEARCH_URL" -o "$ELASTICSEARCH_PORT_9200_TCP" ]; then
	: ${ELASTICSEARCH_URL:=http://elasticsearch:9200}
	export ELASTICSEARCH_URL
    else
	cat >&2 <<EOF
warning: ELASTICSEARCH_URL not set, possible solutions:
   1. Link to an elasticsearch container with:
         docker run --link some-elasticsearch:elasticsearch ...
   2. Set the ELASTICSEARCH_URL like:
         docker run -e ELASTICSEARCH_URL=http://192.168.1.100:9200 ...
   3. Provide the Elastic Search URL on the evebox command line:
         docker run ... jasonish/evebox -e http://192.168.1.100:9200
EOF
    fi

fi

exec "$@"

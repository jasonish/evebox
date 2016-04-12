#! /bin/sh

set -e
set -x

echo "Triggering build on Docker hub."
curl -v -H "Content-Type: application/json" \
     --data '{"build": true}' \
     -X POST https://registry.hub.docker.com/u/jasonish/evebox/trigger/${DOCKER_TRIGGER_TOKEN}/

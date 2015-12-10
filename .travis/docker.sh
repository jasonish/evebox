#! /bin/sh

set -e

echo "Triggering build on Docker hub."
curl -H "Content-Type: application/json" \
     --data '{"build": true}' \
     -X POST https://registry.hub.docker.com/u/jasonish/evebox/trigger/${DOCKER_TRIGGER_TOKEN}/

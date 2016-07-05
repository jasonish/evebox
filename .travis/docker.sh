#! /bin/sh
#
# Trigger a Docker rebuild.

set -e

DOCKER_TRIGGER_URL="https://registry.hub.docker.com/u/jasonish/evebox/trigger"

if [ -e _docker_done ]; then
    echo "After deploy already done, exiting."
    exit 0
fi

if [ "${TRAVIS_REPO_SLUG}" != "${DEPLOY_REPO}" ]; then
    echo "docker: not deploying for repo ${TRAVIS_REPO_SLUG}."
    exit 0
fi

if [ "${TRAVIS_BRANCH}" = "master" ]; then
    echo "Trigger Docker build for latest."
    curl -H "Content-Type: application/json" \
	 --data '{"build": true, "docker_tag": "latest"}' \
	 -X POST ${DOCKER_TRIGGER_URL}/${DOCKER_TRIGGER_TOKEN}/
fi

if [ "${TRAVIS_BRANCH}" = "develop" ]; then
    echo "Trigger Docker build for develop."
    curl -H "Content-Type: application/json" \
	 --data '{"build": true, "docker_tag": "develop"}' \
	 -X POST ${DOCKER_TRIGGER_URL}/${DOCKER_TRIGGER_TOKEN}/
fi

touch _docker_done

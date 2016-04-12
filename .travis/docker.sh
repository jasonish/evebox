#! /bin/sh

set -e
set -x

if [ -e _docker_done ]; then
    echo "After deploy already done, exiting."
    exit 0
fi

if [ "${TRAVIS_REPO_SLUG}" != "${DEPLOY_REPO}" ]; then
    echo "docker: not deploying for repo ${TRAVIS_REPO_SLUG}."
    exit 0
fi

if [ "${TRAVIS_BRANCH}" != "${DEPLOY_BRANCH}" ]; then
    echo "docker: not deploying for branch ${TRAVIS_BRANCH}."
    exit 0
fi

touch _docker_done

echo "Triggering build on Docker hub."
curl -v -H "Content-Type: application/json" \
     --data '{"build": true}' \
     -X POST https://registry.hub.docker.com/u/jasonish/evebox/trigger/${DOCKER_TRIGGER_TOKEN}/

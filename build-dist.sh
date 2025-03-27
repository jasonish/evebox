#! /bin/bash

set -e

BUILD_REV=$(git rev-parse --short HEAD)
GIT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
GIT_TAG=$(git describe --tags --abbrev=0 --exact-match 2> /dev/null || echo "")

echo "===> Git revision: ${BUILD_REV}"
echo "===> Git branch: ${GIT_BRANCH}"
echo "===> Git tag: ${GIT_TAG}"

cross_run() {
    target="$1"
    shift
    if [ "${target}" = "" ]; then
        echo "error: target must be set for cross_run"
        exit 1
    fi
    dockerfile="./docker/builder/Dockerfile.cross"
    tag="private/evebox/builder:cross"
    env
    if [ -z "${GITHUB_REPOSITORY}" -a -t ]; then
        it="-it"
    else
        it=""
    fi
    ${ECHO} docker build \
            --build-arg REAL_UID="$(id -u)" \
            --build-arg REAL_GID="$(id -g)" \
            --cache-from ${tag} \
	    -t ${tag} \
	    -f ${dockerfile} .
    ${ECHO} docker run --rm ${it} --privileged \
            -v "$(pwd):/src:z" \
            -v /var/run/docker.sock:/var/run/docker.sock:z \
            -w /src \
            -e BUILD_REV="${BUILD_REV}" \
            -e TARGET="${target}" \
            -u builder \
            --group-add $(getent group docker | cut -f3 -d:) \
            ${tag} $@
}

cross_run x86_64-unknown-linux-musl make dist
cross_run aarch64-unknown-linux-musl make dist

cross_run x86_64-unknown-linux-musl ./packaging/build-rpm.sh amd64

cross_run x86_64-unknown-linux-musl ./packaging/build-deb.sh amd64
cross_run aarch64-unknown-linux-musl ./packaging/build-deb.sh arm64

cross_run x86_64-pc-windows-gnu make dist

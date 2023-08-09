#! /bin/bash

set -e
set -x

REGISTRY=${REGISTRY:-docker.io}
BUILD_REV=$(git rev-parse --short HEAD)

skip_macos="no"
skip_windows="no"

for a in $@; do
    case "$a" in
        --skip-macos)
            skip_macos="yes"
            ;;
        --skip-windows)
            skip_windows="yes"
            ;;
        --linux)
            skip_macos="yes"
            skip_windows="yes"
            ;;
        *)
            echo "error: bad argument: $a"
            exit 1
            ;;
    esac
done

cross_run() {
    target="$1"
    shift
    if [ "${target}" = "" ]; then
        echo "error: target must be set for cross_run"
        exit 1
    fi
    dockerfile="./docker/builder/Dockerfile.cross"
    tag=${BUILDER_TAG:-"evebox/builder:cross"}
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

build_macos() {
    tag=${BUILDER_TAG:-"evebox/builder:macos"}
    dockerfile="./docker/builder/Dockerfile.macos"
    TARGET="x86_64-apple-darwin"
    docker build \
        --build-arg REAL_UID="$(id -u)" \
        --build-arg REAL_GID="$(id -g)" \
        --cache-from ${tag} \
	-t ${tag} \
	-f ${dockerfile} .
    docker run ${IT} --rm \
        -v "$(pwd):/src:z" \
        -w /src \
        -e CC=o64-clang \
        -e TARGET=${TARGET} \
        -e BUILD_REV="${BUILD_REV}" \
        -u builder \
        --group-add $(getent group docker | cut -f3 -d:) \
        ${tag} make dist
}

cross_run x86_64-unknown-linux-musl make dist
cross_run aarch64-unknown-linux-musl make dist
cross_run arm-unknown-linux-musleabihf make dist

cross_run x86_64-unknown-linux-musl ./packaging/build-rpm.sh amd64

cross_run x86_64-unknown-linux-musl ./packaging/build-deb.sh amd64
cross_run aarch64-unknown-linux-musl ./packaging/build-deb.sh arm64
cross_run aarch64-unknown-linux-musl ./packaging/build-deb.sh arm

if [[ "${skip_windows}" != "yes" ]]; then
    cross_run x86_64-pc-windows-gnu make dist
fi

if [[ "${skip_macos}" != "yes" ]]; then
    build_macos
fi

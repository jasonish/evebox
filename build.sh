#! /bin/bash

set -e

RELEASE="no"
LATEST="no"

if [ "${REGISTRY}" = "" ]; then
    REGISTRY="docker.io"
fi

DOCKER_NAME="${REGISTRY}/jasonish/evebox"

BUILD_REV=$(git rev-parse --short HEAD)
export BUILD_REV

VERSION=$(cat Cargo.toml | awk '/^version/ { gsub(/"/, "", $3); print $3 }')
GIT_BRANCH=$(git rev-parse --abbrev-ref HEAD)

if test -t 1; then
    it="-it"
fi

declare -A COMMANDS

# Set the container tag prefix to "dev" if not on the master branch.
if [ "${DOCKER_TAG_PREFIX}" = "" ]; then
    if [ "${GIT_BRANCH}" = "master" ]; then
        DOCKER_TAG_PREFIX="main"
    elif [ "${GIT_BRANCH}" = "main" ]; then
	DOCKER_TAG_PREFIX="main"
    else
        DOCKER_TAG_PREFIX="dev"
    fi
fi

build_webapp() {
    DOCKERFILE="./docker/builder/Dockerfile.cross"
    TAG=${BUILDER_TAG:-"evebox/builder:webapp"}
    docker build \
           --build-arg REAL_UID="$(id -u)" \
           --build-arg REAL_GID="$(id -g)" \
	   -t ${TAG} \
	   -f ${DOCKERFILE} .
    docker run --rm ${it} \
           -v "$(pwd):/src:z" \
           -w /src \
           -e BUILD_REV="${BUILD_REV}" \
           -u builder \
           --group-add $(getent group docker | cut -f3 -d:) \
           ${TAG} make webapp
}
COMMANDS[webapp]=build_webapp

build_cross() {
    target="$1"
    if [ "${target}" = "" ]; then
        echo "error: target must be set for build_cross"
        exit 1
    fi
    what="$2"
    DOCKERFILE="./docker/builder/Dockerfile.cross"
    TAG=${BUILDER_TAG:-"evebox/builder:cross"}
    docker build \
        --build-arg REAL_UID="$(id -u)" \
        --build-arg REAL_GID="$(id -g)" \
        --cache-from ${TAG} \
	-t ${TAG} \
	-f ${DOCKERFILE} .
    docker run --rm ${it} --privileged \
        -v "$(pwd):/src:z" \
        -v /var/run/docker.sock:/var/run/docker.sock:z \
        -w /src \
        -e BUILD_REV="${BUILD_REV}" \
        -e TARGET="${target}" \
        -u builder \
        --group-add $(getent group docker | cut -f3 -d:) \
        ${TAG} make $what
}

cross_run() {
    target="$1"
    shift
    if [ "${target}" = "" ]; then
        echo "error: target must be set for build_cross"
        exit 1
    fi
    DOCKERFILE="./docker/builder/Dockerfile.cross"
    TAG=${BUILDER_TAG:-"evebox/builder:cross"}
    docker build \
        --build-arg REAL_UID="$(id -u)" \
        --build-arg REAL_GID="$(id -g)" \
        --cache-from ${TAG} \
	-t ${TAG} \
	-f ${DOCKERFILE} .
    docker run --rm ${it} --privileged \
        -v "$(pwd):/src:z" \
        -v /var/run/docker.sock:/var/run/docker.sock:z \
        -w /src \
        -e BUILD_REV="${BUILD_REV}" \
        -e TARGET="${target}" \
        -u builder \
        --group-add $(getent group docker | cut -f3 -d:) \
        ${TAG} $@
}

build_linux() {
    cross_run x86_64-unknown-linux-musl make dist
    cross_run x86_64-unknown-linux-musl ./packaging/build-deb.sh amd64
    cross_run x86_64-unknown-linux-musl ./packaging/build-rpm.sh amd64
}
COMMANDS[linux]=build_linux

build_linux_arm64() {
    cross_run aarch64-unknown-linux-musl make dist
    cross_run aarch64-unknown-linux-musl ./packaging/build-deb.sh arm64
}
COMMANDS[linux-arm64]=build_linux_arm64

build_linux_arm32() {
    cross_run arm-unknown-linux-musleabihf make dist
    cross_run aarch64-unknown-linux-musl ./packaging/build-deb.sh arm
}
COMMANDS[linux-arm32]=build_linux_arm32

build_windows() {
    cross_run x86_64-pc-windows-gnu make dist
}
COMMANDS[windows]=build_windows

build_macos() {
    TAG=${BUILDER_TAG:-"evebox/builder:macos"}
    DOCKERFILE="./docker/builder/Dockerfile.macos"
    TARGET="x86_64-apple-darwin"
    docker build \
        --build-arg REAL_UID="$(id -u)" \
        --build-arg REAL_GID="$(id -g)" \
        --cache-from ${TAG} \
	-t ${TAG} \
	-f ${DOCKERFILE} .
    docker run ${IT} --rm \
        -v "$(pwd):/src:z" \
        -w /src \
        -e CC=o64-clang \
        -e TARGET=${TARGET} \
        -e BUILD_REV="${BUILD_REV}" \
        -u builder \
        --group-add $(getent group docker | cut -f3 -d:) \
        ${TAG} make dist
}

COMMANDS[macos]=build_macos

build_docker() {
    if [[ "${RELEASE}" = "yes" ]]; then
        version=${VERSION}
    else
        version="latest"
    fi

    set -x
    test -e ./dist/evebox-${version}-linux-x64/evebox
    test -e ./dist/evebox-${version}-linux-arm/evebox
    test -e ./dist/evebox-${version}-linux-arm64/evebox
    set +x

    docker build \
	--build-arg "BASE=amd64/alpine" \
        --build-arg "SRC=./dist/evebox-${version}-linux-x64/evebox" \
        -t ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-amd64 \
        -f docker/Dockerfile .

    docker build \
	--build-arg "BASE=arm32v6/alpine" \
        --build-arg "SRC=./dist/evebox-${version}-linux-arm/evebox" \
        -t ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-arm32v6 \
        -f docker/Dockerfile .

    docker build \
	--build-arg "BASE=arm64v8/alpine" \
        --build-arg "SRC=./dist/evebox-${version}-linux-arm64/evebox" \
        -t ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-arm64v8 \
        -f docker/Dockerfile .
}
COMMANDS[docker]=build_docker

docker_push() {
    build_docker

    ${DP_DEBUG} docker push ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-amd64
    ${DP_DEBUG} docker push ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-arm32v6
    ${DP_DEBUG} docker push ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-arm64v8

    ${DP_DEBUG} docker manifest create -a ${DOCKER_NAME}:${DOCKER_TAG_PREFIX} \
        ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-amd64 \
        ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-arm32v6 \
        ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-arm64v8
    ${DP_DEBUG} docker manifest push --purge ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}

    if [ "${LATEST}" = "yes" ]; then
        echo "Pushing Docker image as \"latest\"."
        ${DP_DEBUG} docker manifest create -a ${DOCKER_NAME}:latest \
            ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-amd64 \
            ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-arm32v6 \
            ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-arm64v8
        ${DP_DEBUG} docker manifest push --purge ${DOCKER_NAME}:latest
    fi

    if [ "${DOCKER_TAG_PREFIX}" = "main" ]; then
        echo "Pushing Docker iamge as \"master\"."
        ${DP_DEBUG} docker manifest create -a ${DOCKER_NAME}:master \
            ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-amd64 \
            ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-arm32v6 \
            ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-arm64v8
        ${DP_DEBUG} docker manifest push --purge ${DOCKER_NAME}:master
    fi
}
COMMANDS[docker-push]=docker_push

build_all() {
    build_webapp
    build_linux
    build_linux_arm64
    build_linux_arm32
    build_windows
    build_macos
    build_docker
}

for arg in $@; do
    case "${arg}" in
        --release)
            RELEASE="yes"
            shift
            ;;
        --latest)
            LATEST="yes"
            shift
            ;;
    esac
done

if [ "${RELEASE}" = "yes" ]; then
    DOCKER_TAG_PREFIX="${VERSION}"
fi

if [[ "${1}" ]]; then
    if [[ "${1}" == "all" ]]; then
        build_all
        exit 0
    else
        command=${COMMANDS[${1}]}
        if [[ "${command}" ]]; then
            ${COMMANDS[${1}]}
            exit 0
        fi
    fi
    echo "Error: Unknown command: $1"
    exit 1
fi

echo "Commands:"
for key in "${!COMMANDS[@]}"; do
    echo "    ${key}"
done

#! /bin/bash

set -e

if [ "${REGISTRY}" = "" ]; then
    REGISTRY="docker.io"
fi

DOCKER_NAME="${REGISTRY}/jasonish/evebox"

BUILD_REV=$(git rev-parse --short HEAD)
export BUILD_REV

VERSION=$(cat Cargo.toml | awk '/^version/ { gsub(/"/, "", $3); print $3 }')
GIT_BRANCH=$(git rev-parse --abrev-ref HEAD)

# Set the container tag prefix to "dev" if not on the master branch.
if [ "${DOCKER_TAG_PREFIX}" = "" ]; then
    if [ "${GIT_BRANCH}" = "master" ]; then
        DOCKER_TAG_PREFIX="master"
    else
        DOCKER_TAG_PREFIX="dev"
    fi
fi

echo "BUILD_REV=${BUILD_REV}"

build_webapp() {
    DOCKERFILE="./docker/builder/Dockerfile.musl"
    TAG=${BUILDER_TAG:-"evebox/builder:webapp"}
    docker build ${CACHE_FROM} --rm \
           --build-arg REAL_UID="$(id -u)" \
           --build-arg REAL_GID="$(id -g)" \
           --cache-from ${TAG} \
	   -t ${TAG} \
	   -f ${DOCKERFILE} .
    docker run ${IT} --rm \
           -v "$(pwd):/src" \
           -w /src/webapp \
           -e REAL_UID="$(id -u)" \
           -e REAL_GID="$(id -g)" \
           -e BUILD_REV="${BUILD_REV}" \
           ${TAG} make
}

# Linux - x86_64
build_linux() {
    DOCKERFILE="./docker/builder/Dockerfile.musl"
    TAG=${BUILDER_TAG:-"evebox/builder:musl"}
    docker build --rm \
           --build-arg REAL_UID="$(id -u)" \
           --build-arg REAL_GID="$(id -g)" \
           --cache-from ${TAG} \
	   -t ${TAG} \
	   -f ${DOCKERFILE} .
    docker run ${IT} --rm \
           -v "$(pwd):/src" \
           -v "$HOME/.cargo:/home/builder/.cargo" \
           -w /src \
           -e REAL_UID="$(id -u)" \
           -e REAL_GID="$(id -g)" \
           -e BUILD_REV="${BUILD_REV}" \
           -e TARGET="x86_64-unknown-linux-musl" \
           ${TAG} make dist rpm deb
}

build_linux_armv7() {
    DOCKERFILE="./docker/builder/Dockerfile.armv7"
    TAG=${BUILDER_TAG:-"evebox/builder:armv7"}
    docker build --rm \
           --cache-from ${TAG} \
	   -t ${TAG} \
	   -f ${DOCKERFILE} .
    docker run ${IT} --rm \
         -v "$(pwd)/target:/src/target:z" \
         -v "$(pwd)/dist:/src/dist:z" \
         -v /var/run/docker.sock:/var/run/docker.sock \
         -w /src \
         -e REAL_UID="$(id -u)" \
         -e REAL_GID="$(id -g)" \
         -e BUILD_REV="${BUILD_REV}" \
         -e TARGET="armv7-unknown-linux-musleabihf" \
         -e CARGO="cross" \
         ${TAG} make dist
}


build_windows() {
    TAG=${BUILDER_TAG:-"evebox/builder:windows"}
    DOCKERFILE="./docker/builder/Dockerfile.windows"
    docker build ${CACHE_FROM} --rm \
           --build-arg REAL_UID="$(id -u)" \
           --build-arg REAL_GID="$(id -g)" \
           --cache-from ${TAG} \
	   -t ${TAG} \
	   -f ${DOCKERFILE} .
    docker run ${IT} --rm \
           -v "$(pwd):/src" \
           -w /src \
           -e REAL_UID="$(id -u)" \
           -e REAL_GID="$(id -g)" \
           -e CC=x86_64-w64-mingw32-gcc \
           -e TARGET=x86_64-pc-windows-gnu \
           -e BUILD_REV="${BUILD_REV}" \
           ${TAG} make dist
}

build_macos() {
    TAG=${BUILDER_TAG:-"evebox/builder:macos"}
    DOCKERFILE="./docker/builder/Dockerfile.macos"
    docker build ${CACHE_FROM} --rm \
           --build-arg REAL_UID="$(id -u)" \
           --build-arg REAL_GID="$(id -g)" \
           --cache-from ${TAG} \
	   -t ${TAG} \
	   -f ${DOCKERFILE} .
    docker run ${IT} --rm \
           -v "$(pwd):/src" \
           -w /src \
           -e REAL_UID="$(id -u)" \
           -e REAL_GID="$(id -g)" \
           -e CC=o64-clang \
           -e TARGET=x86_64-apple-darwin \
           -e BUILD_REV="${BUILD_REV}" \
           ${TAG} make dist
}

build_docker() {
    if test -e ./dist/evebox-${VERSION}-linux-x64/evebox; then
        version=${VERSION}
    else
        version="latest"
    fi
    docker build \
	   --build-arg "BASE=amd64/alpine" \
           --build-arg "SRC=./dist/evebox-${version}-linux-x64/evebox" \
           -t ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-amd64 \
           -f docker/Dockerfile .

    docker build \
	   --build-arg "BASE=arm32v7/alpine" \
           --build-arg "SRC=./dist/evebox-${version}-linux-arm/evebox" \
           -t ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-arm32v7 \
           -f docker/Dockerfile .
}

docker_push() {
    docker push ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-amd64
    docker push ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-arm32v7

    docker manifest create -a ${DOCKER_NAME}:${DOCKER_TAG_PREFIX} \
           ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-amd64 \
           ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-arm32v7
    docker manifest annotate --arch arm --variant v7 \
           ${DOCKER_NAME}:${DOCKER_TAG_PREFIX} \
           ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}-arm32v7
    docker manifest push --purge ${DOCKER_NAME}:${DOCKER_TAG_PREFIX}
}

build_all() {
    rm -rf dist

    build_webapp
    ./docker.sh release-linux
    ./docker.sh release-windows
    ./docker.sh release-macos
}

push() {
    (cd dist && sha256sum *.zip *.rpm *.deb > CHECKSUMS.txt)

    if [ "${EVEBOX_RSYNC_PUSH_DEST}" ]; then
        rsync -av \
              --filter "+ *.rpm" \
              --filter "+ *.deb" \
              --filter "+ *.zip" \
              --filter "+ CHECKSUMS.txt" \
              --filter "- *" \
              dist/ \
              "${EVEBOX_RSYNC_PUSH_DEST}"
    else
        echo "error: EVEBOX_RSYNC_PUSH_DEST environment variable not set"
    fi
}

case "$1" in
    webapp)
        build_webapp
        ;;

    linux)
        build_linux
        ;;

    linux-arm)
        build_linux_armv7
        ;;

    windows)
        build_windows
        ;;

    macos)
        build_macos
        ;;

    docker)
        build_docker
        ;;

    docker-push)
        build_docker
        docker_push
        ;;

    push)
        push
        ;;

    all)
        build_webapp
        build_linux
        build_linux_armv7
        build_windows
        build_macos
        build_docker
        ;;

    *)
        cat <<EOF
usage: $0 <command>

Commands:
    release-linux      Build x86_64 Linux release - zip/deb/rpm.
    release-arm7       Build arm7 Linux Release (RPi) - zip
    all
EOF
        exit 1
        ;;
esac

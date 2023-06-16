#! /bin/bash
#
# Arguments:
#
# --skip-make       Skips building (making) EveBox
# --push            Push images and manifest
# --latest          Also tag as "latest"
#
# Variables:
#
# version           EveBox version to containerize, default tag
# tag               Override the tag which defaults to the version

set -e

REGISTRY=${REGISTRY:-docker.io}
BUILD_REV=$(git rev-parse --short HEAD)
DOCKER_NAME="${REGISTRY}/jasonish/evebox"

push="no"
skip_make="no"
latest="no"

aliases=()

for a in $@; do
    case "$a" in
        --push)
            push="yes"
            ;;
        --skip-make)
            skip_make="yes"
            ;;
        --latest)
            latest="yes"
            aliases+=("latest")
            ;;
        *)
            echo "error: bad argument: $a"
            exit 1
            ;;
    esac
done

if [[ "${version}" = "" ]]; then
    echo "error: version must be set"
    exit 1
fi

if [[ "${tag}" = "" ]]; then
    case "${version}" in
        devel)
            tag=${version}
            aliases+=("master")
            ;;
        *)
            tag=${version}
            ;;
    esac
fi

cross_run() {
    target="$1"
    shift
    if [ "${target}" = "" ]; then
        echo "error: target must be set for cross_run"
        exit 1
    fi
    DOCKERFILE="./docker/builder/Dockerfile.cross"
    TAG=${BUILDER_TAG:-"evebox/builder:cross"}
    ${ECHO} docker build \
        --build-arg REAL_UID="$(id -u)" \
        --build-arg REAL_GID="$(id -g)" \
        --cache-from ${TAG} \
	-t ${TAG} \
	-f ${DOCKERFILE} .
    ${ECHO} docker run --rm ${it} --privileged \
        -v "$(pwd):/src:z" \
        -v /var/run/docker.sock:/var/run/docker.sock:z \
        -w /src \
        -e BUILD_REV="${BUILD_REV}" \
        -e TARGET="${target}" \
        -u builder \
        --group-add $(getent group docker | cut -f3 -d:) \
        ${TAG} $@
}

if [[ "${skip_make}" = "yes" ]]; then
    echo "===> Skipping make of EveBox"
else
    cross_run x86_64-unknown-linux-musl    make dist
    cross_run aarch64-unknown-linux-musl   make dist
    cross_run arm-unknown-linux-musleabihf make dist
fi

bins=(
    ./dist/evebox-${version}-linux-x64/evebox
    ./dist/evebox-${version}-linux-arm64/evebox
    ./dist/evebox-${version}-linux-arm/evebox
)

for bin in ${bins}; do
    if ! test -e ${bin}; then
        echo "error: ${bin} does not exist"
        exit 1
    fi
done

${ECHO} docker build \
       --build-arg "BASE=amd64/alpine" \
       --build-arg "SRC=./dist/evebox-${version}-linux-x64/evebox" \
       -t ${DOCKER_NAME}:${tag}-amd64 \
       -f docker/Dockerfile .

${ECHO} docker build \
       --build-arg "BASE=arm32v6/alpine" \
       --build-arg "SRC=./dist/evebox-${version}-linux-arm/evebox" \
       -t ${DOCKER_NAME}:${tag}-arm32v6 \
       -f docker/Dockerfile .

${ECHO} docker build \
       --build-arg "BASE=arm64v8/alpine" \
       --build-arg "SRC=./dist/evebox-${version}-linux-arm64/evebox" \
       -t ${DOCKER_NAME}:${tag}-arm64v8 \
       -f docker/Dockerfile .

if [[ "${push}" = "yes" ]]; then
    ${ECHO} docker push ${DOCKER_NAME}:${tag}-amd64
    ${ECHO} docker push ${DOCKER_NAME}:${tag}-arm32v6
    ${ECHO} docker push ${DOCKER_NAME}:${tag}-arm64v8

    ${ECHO} docker manifest create -a ${DOCKER_NAME}:${tag} \
        ${DOCKER_NAME}:${tag}-amd64 \
        ${DOCKER_NAME}:${tag}-arm32v6 \
        ${DOCKER_NAME}:${tag}-arm64v8
    ${ECHO} docker manifest push --purge ${DOCKER_NAME}:${tag}

    for alias in ${aliases}; do
        ${ECHO} docker manifest create -a ${DOCKER_NAME}:${alias} \
            ${DOCKER_NAME}:${tag}-amd64 \
            ${DOCKER_NAME}:${tag}-arm32v6 \
            ${DOCKER_NAME}:${tag}-arm64v8
        ${ECHO} docker manifest push --purge ${DOCKER_NAME}:${alias}
    done
fi

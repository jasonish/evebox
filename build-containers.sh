#! /bin/bash
#
# Arguments:
#
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
GIT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
GIT_TAG=$(git describe --tags --abbrev=0 --exact-match 2> /dev/null || echo "")

push="no"
latest="no"

aliases=()

for a in $@; do
    case "$a" in
        --push)
            push="yes"
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

if [[ "${GIT_TAG}" ]]; then
    echo "Building container for version ${GIT_TAG}"
    version="${GIT_TAG}"
elif [[ "${GIT_BRANCH}" = "devel" ]]; then
    version="devel"
    tag="devel"
elif [[ "${GIT_BRANCH}" = "main" ]]; then
    echo "Building devel version from branch main"
    version="devel"
    tag="main"
    aliases+=("master")
else
    echo "Not building from branch or tag."
    echo "  - version and tag must be set"
fi

echo "BRANCH=${GIT_BRANCH}"

if [[ "${version}" = "" ]]; then
    echo "error: version must be set"
    exit 1
fi

if [[ "${tag}" = "" ]]; then
    echo "error: tag must be set"
    exit 1
fi

if [[ "${tag}" = "master" ]]; then
    echo "===> Will also push as 'main'"
    aliases+=("main")
fi

if [[ "${tag}" = "main" ]]; then
    echo "===> Will also push as 'master'"
    aliases+=("master")
fi

bins=(
    ./dist/evebox-${version}-linux-x64/evebox
    ./dist/evebox-${version}-linux-arm64/evebox
)

for bin in ${bins}; do
    if ! test -e ${bin}; then
        echo "error: ${bin} does not exist"
        exit 1
    fi
done

tags_built=()
tags_pushed=()
manifests_pushed=()

name="${DOCKER_NAME}:${tag}-amd64"
${ECHO} docker build \
       --build-arg "BASE=amd64/almalinux:9-minimal" \
       --build-arg "SRC=./dist/evebox-${version}-linux-x64/evebox" \
       -t ${name} \
       -f docker/Dockerfile .
tags_built+=(${name})

name="${DOCKER_NAME}:${tag}-arm64v8"
${ECHO} docker build \
       --build-arg "BASE=arm64v8/almalinux:9-minimal" \
       --build-arg "SRC=./dist/evebox-${version}-linux-arm64/evebox" \
       -t ${name} \
       -f docker/Dockerfile .
tags_built+=(${name})

function push() {
    source=$1
    echo "Pushing ${source}"
    ${ECHO} docker push ${source}
    tags_pushed+=(${source})
}

function push_manifest() {
    manifest=$1
    echo "Pushing ${manifest}"
    ${ECHO} docker manifest push --purge ${manifest}
    manifests_pushed+=(${manifest})
}

if [[ "${push}" = "yes" ]]; then
    push ${DOCKER_NAME}:${tag}-amd64
    push ${DOCKER_NAME}:${tag}-arm64v8

    ${ECHO} docker manifest create -a ${DOCKER_NAME}:${tag} \
        ${DOCKER_NAME}:${tag}-amd64 \
        ${DOCKER_NAME}:${tag}-arm64v8
    push_manifest ${DOCKER_NAME}:${tag}

    for alias in ${aliases}; do
        ${ECHO} docker manifest create -a ${DOCKER_NAME}:${alias} \
            ${DOCKER_NAME}:${tag}-amd64 \
            ${DOCKER_NAME}:${tag}-arm64v8
        push_manifest ${DOCKER_NAME}:${alias}
    done

    echo "Tags pushed:"
    for tag in ${tags_pushed[@]}; do
        echo "  ${tag}"
    done

    echo "Manifests pushed:"
    for manifest in ${manifests_pushed[@]}; do
        echo "  ${manifest}"
    done

else
    echo ""
    echo "Re-run with --push to push."
    echo ""
fi


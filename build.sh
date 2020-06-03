#! /bin/sh

set -e

DOCKER_NAME="jasonish/evebox"
BRANCH_PREFIX=$(git rev-parse --abbrev-ref HEAD | awk '{split($0,a,"/"); print a[1]}')

build_docker() {
    docker build -t ${DOCKER_NAME}:${BRANCH_PREFIX} .
}

build_all() {
    rm -rf dist

    build_docker
    ./docker.sh webapp
    ./docker.sh release-linux
    ./docker.sh release-windows
    ./docker.sh release-macos
}

push() {
    docker push ${DOCKER_NAME}:${BRANCH_PREFIX}

    (cd dist && sha256sum *.zip *.rpm *.deb > CHECKSUMS.txt)

    if [ "${EVEBOX_RSYNC_PUSH_DEST}" ]; then
        rsync -av --delete --delete-excluded \
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
    docker)
        build_docker
        ;;
    
    all)
        build_all
        ;;

    push)
        push
        ;;

    *)
        cat <<EOF
usage: $0 <command>

Commands:
    all
EOF
        ;;
esac

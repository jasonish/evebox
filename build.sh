#! /bin/sh

set -e

BRANCH_PREFIX=$(git rev-parse --abbrev-ref HEAD | awk '{split($0,a,"/"); print a[1]}')

build_docker() {
    docker build -t jasonish/evebox:${BRANCH_PREFIX} .
}

build_all() {
    rm -rf dist

    build_docker
    ./docker.sh webapp
    ./docker.sh release-linux
    ./docker.sh release-windows
    ./docker.sh release-macos
}

case "$1" in
    docker)
        build_docker
        ;;
    
    all)
        build_all
        ;;

    *)
        cat <<EOF
usage: $0 <command>

Commands:
    all
EOF
        ;;
esac

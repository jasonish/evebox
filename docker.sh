#! /usr/bin/env bash

docker_build() {
    docker build --rm -t evebox/builder -f ./docker/builder/Dockerfile .
}

docker_run() {
    mkdir -p $(pwd)/.docker_cache/go
    mkdir -p $(pwd)/.docker_cache/node_modules
    mkdir -p $(pwd)/.docker_cache/npm
    docker run --rm -it \
	   -v $(pwd):/src \
	   -v $(pwd)/.docker_cache/go:/home/builder/go \
	   -v $(pwd)/.docker_cache/npm:/home/builder/.npm \
	   -v $(pwd)/.docker_cache/node_modules:/src/webapp/node_modules \
	   -e REAL_UID=$(id -u) \
	   -e REAL_GID=$(id -g) \
	   -w /src \
	   -u builder \
	   evebox/builder "$@"
}

needs_privilege() {
    if [ "$(getenforce || true)" = "Enforcing" ]; then
	return 0
    else
	return 1
    fi
}

release() {
    docker_build
    docker_run make install-deps dist rpm deb
}

case "$1" in

    release)
	release
	;;

    install-deps)
	docker_build
	docker_run make install-deps
	;;

    make)
	docker_build
	docker_run make
	;;

    shell)
	docker_build
	docker_run
	;;

    *)
	if [[ "$1" ]]; then
	    docker_build
	    docker_run "$@"
	else
	cat <<EOF
usage: ./docker.sh <command>

Commands:
    release            Build a release
EOF
	fi
	;;

esac

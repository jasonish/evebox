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
	   evebox/builder "$@"
}

release() {
    docker_build
    docker_run make install-deps dist rpm deb
}

release_windows() {
    docker_build
    docker_run "make install-deps"
    docker_run "GOOS=windows CC=x86_64-w64-mingw32-gcc make dist"
}

case "$1" in

    release)
	release
	;;

    release-windows)
	release_windows
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
    release            Build x86_64 Linux release - zip/deb/rpm.
    release-windows    Build x86_64 Windows release zip.
EOF
	fi
	;;

esac

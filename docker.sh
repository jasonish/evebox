#! /bin/sh

set -e

IMAGE="evebox/builder:latest"

docker_build() {
    docker build ${CACHE_FROM} --rm \
	   -t ${IMAGE} \
	   -f ${DOCKERFILE} .
}

docker_run() {
    # If you need an interactive terminal you can set the environment
    # variable "it" to "-it".
    volumes=""
    volumes="${volumes} -v $(pwd):/src"

    cache="$(pwd)/.docker_cache"

    mkdir -p ${cache}/go
    mkdir -p ${cache}/node_modules
    mkdir -p ${cache}/npm
    mkdir -p ./webapp/node_modules

    real_uid=$(id -u)
    real_gid=$(id -g)

    if [ "${real_uid}" = "0" ]; then
	image_home="/root"
    else
	image_home="/home/builder"
    fi

    volumes="${volumes} -v ${cache}/go:${image_home}/go"
    volumes="${volumes} -v ${cache}/npm:${image_home}/npm"
    #volumes="${volumes} -v ${cache}/node_modules:/src/webapp/node_modules"

    docker run --rm ${it} ${volumes} \
	   -e REAL_UID="${real_uid}" \
	   -e REAL_GID="${real_gid}" \
	   -w /src \
	   "${IMAGE}" "$@"
}

release() {
    DOCKERFILE="./docker/builder/Dockerfile"
    docker_build
    docker_run "make install-deps dist rpm deb"
}

release_windows() {
    IMAGE="evebox/builder:windows"
    DOCKERFILE="./docker/builder/Dockerfile.windows"
    docker_build
    docker_run \
	"make install-deps && GOOS=windows CC=x86_64-w64-mingw32-gcc make dist"
}

release_macos() {
    IMAGE="evebox/builder:macos"
    DOCKERFILE="./docker/builder/Dockerfile.macos"
    docker_build
    docker_run \
	 "make install-deps && GOOS=darwin CC=o64-clang make dist"
}

case "$1" in

    release)
	release
	;;

    release-windows)
	release_windows
	;;

    release-macos)
	release_macos
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
	if [ "$1" ]; then
	    docker_build
	    docker_run "$@"
	else
	cat <<EOF
usage: ./docker.sh <command>

Commands:
    release            Build x86_64 Linux release - zip/deb/rpm.
    release-windows    Build x86_64 Windows release zip.
    release-macos      Build x86_64 MacOS release zip.
EOF
	fi
	;;

esac

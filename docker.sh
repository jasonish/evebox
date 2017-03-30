#! /bin/sh

docker_build() {
    docker build --rm -t evebox/builder - < Dockerfile
}

docker_run() {
    docker run --rm -it \
	   -v $(pwd):/go/src/github.com/jasonish/evebox \
	   -w /go/src/github.com/jasonish/evebox \
	   -e WITH_SQLITE=${WITH_SQLITE} \
	   --privileged \
	   evebox/builder $@
}

needs_privilege() {
    if [ "$(getenforce || true)" = "Enforcing" ]; then
	return 0
    else
	return 1
    fi
}

release() {
    #docker_build

    privileged=""
    if needs_privilege; then
	privileged="--privileged"
    fi

    if [ -e ./dist ]; then
	echo "Deleting exist ./dist directory."
	rm -rf ./dist
    fi

    docker build --rm -t evebox/release-builder \
	   -f ./docker/release-builder/Dockerfile .
    docker run --rm -it ${privileged} \
	   -e REAL_UID=$(id -u) -e REAL_GID=$(id -g) \
	   -v $(pwd)/dist:/dist \
	   evebox/release-builder
}

case "$1" in

    release)
	release
	;;

    install-deps)
	docker_build
	docker_run make install-deps
	;;

    build)
	docker_build
	docker_run make
	;;

    shell)
	docker_build
	docker_run
	;;

    *)
	cat <<EOF
usage: ./docker.sh <command>

Commands:
    release            Build a release
    install-deps       Install deps (make install-deps)
    build              Build evebox (make)
    shell              Execute shell
EOF
	;;

esac

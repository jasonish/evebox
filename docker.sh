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

release() {
    docker_build
    docker run --rm -it \
	   -v $(pwd):/go/src/github.com/jasonish/evebox \
	   -w /go/src/github.com/jasonish/evebox \
	   --privileged \
	   evebox/builder \
	   make install-deps release deb rpm
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

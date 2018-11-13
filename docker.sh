#! /bin/sh

release() {
    if [ -e ./dist ]; then
	echo "Deleting exist ./dist directory."
	rm -rf ./dist
    fi

    docker build --rm -t evebox/release-builder \
	   -f ./docker/release-builder/Dockerfile .
    docker run --rm -it \
	   -e REAL_UID=$(id -u) -e REAL_GID=$(id -g) \
	   -v $(pwd)/dist:/dist \
	   evebox/release-builder
}

case "$1" in

    release)
	release
	;;

    *)
	cat <<EOF
usage: ./docker.sh <command>

Commands:
    release            Build a release
EOF
	;;

esac

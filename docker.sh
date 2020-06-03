#! /bin/sh

set -e

BUILD_REV=$(git rev-parse --short HEAD)
export BUILD_REV

webapp() {
    DOCKERFILE="./docker/builder/Dockerfile"
    TAG="evebox/builder:linux"
    docker build ${CACHE_FROM} --rm \
           --build-arg REAL_UID="$(id -u)" \
           --build-arg REAL_GID="$(id -g)" \
	   -t ${TAG} \
	   -f ${DOCKERFILE} .
    docker run ${IT} --rm \
           -v "$(pwd):/src" \
           -w /src/webapp \
           -e REAL_UID="$(id -u)" \
           -e REAL_GID="$(id -g)" \
           -e BUILD_REV="${BUILD_REV}" \
           ${TAG} make
}

release_musl() {
    DOCKERFILE="./docker/builder/Dockerfile.musl"
    TAG="evebox/builder:musl"
    docker build --rm \
           --build-arg REAL_UID="$(id -u)" \
           --build-arg REAL_GID="$(id -g)" \
	   -t ${TAG} \
	   -f ${DOCKERFILE} .
    docker run ${IT} --rm \
           -v "$(pwd):/src" \
           -v "$HOME/.cargo:/home/builder/.cargo" \
           -w /src \
           -e REAL_UID="$(id -u)" \
           -e REAL_GID="$(id -g)" \
           -e BUILD_REV="${BUILD_REV}" \
           -e TARGET="x86_64-unknown-linux-musl" \
           ${TAG} make dist rpm deb
}

release_windows() {
    TAG="evebox/builder:windows"
    DOCKERFILE="./docker/builder/Dockerfile.windows"
    docker build ${CACHE_FROM} --rm \
           --build-arg REAL_UID="$(id -u)" \
           --build-arg REAL_GID="$(id -g)" \
	   -t ${TAG} \
	   -f ${DOCKERFILE} .
    docker run ${IT} --rm \
           -v "$(pwd):/src" \
           -w /src \
           -e REAL_UID="$(id -u)" \
           -e REAL_GID="$(id -g)" \
           -e CC=x86_64-w64-mingw32-gcc \
           -e TARGET=x86_64-pc-windows-gnu \
           -e BUILD_REV="${BUILD_REV}" \
           ${TAG} make dist
}

release_macos() {
    TAG="evebox/builder:macos"
    DOCKERFILE="./docker/builder/Dockerfile.macos"
    docker build ${CACHE_FROM} --rm \
           --build-arg REAL_UID="$(id -u)" \
           --build-arg REAL_GID="$(id -g)" \
	   -t ${TAG} \
	   -f ${DOCKERFILE} .
    docker run ${IT} --rm \
           -v "$(pwd):/src" \
           -w /src \
           -e REAL_UID="$(id -u)" \
           -e REAL_GID="$(id -g)" \
           -e CC=o64-clang \
           -e TARGET=x86_64-apple-darwin \
           -e BUILD_REV="${BUILD_REV}" \
           ${TAG} make dist
}

case "$1" in

    webapp)
        webapp
        ;;

    release|release-linux|release-musl)
	release_musl
	;;

    release-musl)
	release_musl
	;;

    release-windows)
	release_windows
	;;

    release-macos)
	release_macos
	;;

    *)
	cat <<EOF
usage: ./docker.sh <command>

Commands:
    webapp             Just build the web application
    release-linux      Build x86_64 Linux release - zip/deb/rpm.
    release-windows    Build x86_64 Windows release zip.
    release-macos      Build x86_64 MacOS release zip.
EOF
	;;

esac

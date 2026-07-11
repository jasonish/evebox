#! /bin/bash

set -e

BUILD_REV=$(git rev-parse --short HEAD)
GIT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
GIT_TAG=$(git describe --tags --abbrev=0 --exact-match 2> /dev/null || echo "")

echo "===> Git revision: ${BUILD_REV}"
echo "===> Git branch: ${GIT_BRANCH}"
echo "===> Git tag: ${GIT_TAG}"

# Linux release builds use the target-specific messense/rust-musl-cross
# toolchain images directly. Each image contains a static-only libpcap in the
# musl sysroot; no cross-rs or nested Docker is involved.
musl_build_image() {
    musl_target="$1"
    tag="$2"
    ${ECHO} docker build \
        --build-arg MUSL_TARGET="${musl_target}" \
        --build-arg REAL_UID="$(id -u)" \
        --build-arg REAL_GID="$(id -g)" \
        -t "${tag}" \
        -f ./docker/builder/Dockerfile.musl .
}

musl_run() {
    tag="$1"
    shift
    if [ -z "${GITHUB_REPOSITORY}" -a -t 1 ]; then
        it="-it"
    else
        it=""
    fi

    # The container is removed after each command, so preserve Cargo's
    # downloaded registry and git sources on the host. Mount only these cache
    # directories: the image's target-specific Cargo config must remain in
    # /home/builder/.cargo.
    host_cargo_home="${EVEBOX_CARGO_CACHE_DIR:-${CARGO_HOME:-${HOME}/.cargo}}"
    mkdir -p "${host_cargo_home}/registry" "${host_cargo_home}/git"

    ${ECHO} docker run --rm ${it} \
        -v "$(pwd):/src:z" \
        -v "${host_cargo_home}/registry:/home/builder/.cargo/registry:z" \
        -v "${host_cargo_home}/git:/home/builder/.cargo/git:z" \
        -w /src \
        -e BUILD_REV="${BUILD_REV}" \
        -u builder \
        "${tag}" "$@"
}

# cross-rs is retained for the Windows build only.
cross_run() {
    target="$1"
    shift
    if [ "${target}" = "" ]; then
        echo "error: target must be set for cross_run"
        exit 1
    fi
    dockerfile="./docker/builder/Dockerfile.cross"
    tag="private/evebox/builder:cross"
    if [ -z "${GITHUB_REPOSITORY}" -a -t ]; then
        it="-it"
    else
        it=""
    fi
    ${ECHO} docker build \
            --build-arg REAL_UID="$(id -u)" \
            --build-arg REAL_GID="$(id -g)" \
	    -t ${tag} \
	    -f ${dockerfile} .
    ${ECHO} docker run --rm ${it} --privileged \
            -v "$(pwd):/src:z" \
            -v /var/run/docker.sock:/var/run/docker.sock:z \
            -w /src \
            -e BUILD_REV="${BUILD_REV}" \
            -e TARGET="${target}" \
            -u builder \
            --group-add $(getent group docker | cut -f3 -d:) \
            ${tag} $@
}

build_linux_x64() {
    echo "===> Building Linux x64 (dist + RPM + Debian)"
    tag="private/evebox/builder:musl-x86_64"
    musl_build_image x86_64-musl "${tag}"
    musl_run "${tag}" make dist TARGET=x86_64-unknown-linux-musl
    musl_run "${tag}" ./packaging/build-rpm.sh amd64
    musl_run "${tag}" ./packaging/build-deb.sh amd64
}

build_linux_arm64() {
    echo "===> Building Linux ARM64 (dist + Debian)"
    tag="private/evebox/builder:musl-aarch64"
    musl_build_image aarch64-musl "${tag}"
    musl_run "${tag}" make dist TARGET=aarch64-unknown-linux-musl
    musl_run "${tag}" ./packaging/build-deb.sh arm64
}

build_windows_x64() {
    echo "===> Building Windows x64 (dist)"
    cross_run x86_64-pc-windows-gnu make dist
}

# Parse arguments
if [ $# -eq 0 ]; then
    # No arguments: run everything (backward compatible)
    build_linux_x64
    build_linux_arm64
    build_windows_x64
else
    # Run specific targets
    for target in "$@"; do
        case "$target" in
            linux-x64)
                build_linux_x64
                ;;
            linux-arm64)
                build_linux_arm64
                ;;
            windows-x64)
                build_windows_x64
                ;;
            *)
                echo "Unknown target: $target"
                echo "Valid targets: linux-x64, linux-arm64, windows-x64"
                exit 1
                ;;
        esac
    done
fi

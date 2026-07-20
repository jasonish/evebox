#! /bin/bash

set -e

# Build and archive one release target. This runs inside the target-specific
# builder container selected by the public build functions below.
build_archive() {
    rust_target="$1"

    cargo_version=$(
        awk -F '"' '/^version[[:space:]]*=/ { print $2; exit }' Cargo.toml
    )
    case "${cargo_version}" in
        *-*) dist_version="devel" ;;
        *) dist_version="${cargo_version}" ;;
    esac

    case "${rust_target}" in
        x86_64-unknown-linux-musl)
            platform="linux-x64"
            app="evebox"
            ;;
        aarch64-unknown-linux-musl)
            platform="linux-arm64"
            app="evebox"
            ;;
        x86_64-pc-windows-gnu)
            platform="windows-x64"
            app="evebox.exe"
            ;;
        *)
            echo "error: unsupported archive target: ${rust_target}"
            exit 1
            ;;
    esac

    dist_name="evebox-${dist_version}-${platform}"
    dist_dir="dist/${dist_name}"

    echo "===> Building ${dist_name}"
    make -C webapp
    "${CARGO:-cargo}" build --release --target "${rust_target}"
    mkdir -p "${dist_dir}/examples"
    cp "target/${rust_target}/release/${app}" "${dist_dir}/"
    cp examples/agent.yaml examples/evebox.yaml "${dist_dir}/examples/"
    (cd dist && zip -r "${dist_name}.zip" "${dist_name}")
}

# --archive is an internal mode run inside a builder container.
if [ "${1:-}" = "--archive" ]; then
    if [ "$#" -ne 2 ]; then
        echo "error: --archive requires one Rust target"
        exit 1
    fi
    build_archive "$2"
    exit 0
fi

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
    musl_run "${tag}" ./packaging/build-dist.sh --archive \
        x86_64-unknown-linux-musl
    musl_run "${tag}" ./packaging/build-rpm.sh amd64
    musl_run "${tag}" ./packaging/build-deb.sh amd64
}

# Build only the artifact needed as input to the x86_64 container image.
# Unlike linux-x64, this skips RPM and Debian package generation.
build_linux_x64_container() {
    echo "===> Building Linux x64 (container input)"
    tag="private/evebox/builder:musl-x86_64"
    musl_build_image x86_64-musl "${tag}"
    musl_run "${tag}" ./packaging/build-dist.sh --archive \
        x86_64-unknown-linux-musl
}

build_linux_arm64() {
    echo "===> Building Linux ARM64 (dist + Debian)"
    tag="private/evebox/builder:musl-aarch64"
    musl_build_image aarch64-musl "${tag}"
    musl_run "${tag}" ./packaging/build-dist.sh --archive \
        aarch64-unknown-linux-musl
    musl_run "${tag}" ./packaging/build-deb.sh arm64
}

build_windows_x64() {
    echo "===> Building Windows x64 (dist)"
    cross_run x86_64-pc-windows-gnu ./packaging/build-dist.sh --archive \
        x86_64-pc-windows-gnu
}

# Parse public target arguments.
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
            linux-x64-container)
                build_linux_x64_container
                ;;
            linux-arm64)
                build_linux_arm64
                ;;
            windows-x64)
                build_windows_x64
                ;;
            *)
                echo "Unknown target: $target"
                echo "Valid targets: linux-x64, linux-x64-container, linux-arm64, windows-x64"
                exit 1
                ;;
        esac
    done
fi

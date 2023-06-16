#! /bin/sh

set -e
set -x

CARGO_VERSION=$(cat Cargo.toml | awk '/^version/ { gsub(/"/, "", $3); print $3 }')
VERSION=$(echo ${CARGO_VERSION} | sed 's/\(.*\)\-.*/\1/')
VERSION_SUFFIX=$(echo ${CARGO_VERSION} | sed -n 's/.*-\(.*\)/\1/p')
DATE=$(date +%s)

if [ "${VERSION_SUFFIX}" != "" ]; then
    BIN_SRC_VER="devel"
else
    BIN_SRC_VER="${VERSION}"
fi

case "${1}" in
    "x86_64"|"amd64")
	ARCH="amd64"
	BIN="./dist/evebox-${BIN_SRC_VER}-linux-x64/evebox"
	;;
    "arm64")
	ARCH="arm64"
	BIN="./dist/evebox-${BIN_SRC_VER}-linux-arm64/evebox"
	;;
    "arm")
	ARCH="armhf"
	BIN="./dist/evebox-${BIN_SRC_VER}-linux-arm/evebox"
	;;
    *)
	echo "error: invalid ARCH"
	exit 1
esac

if [ "${VERSION_SUFFIX}" ]; then
    VERSION="${VERSION}~${VERSION_SUFFIX}${DATE}"
fi

if [ "${VERSION_SUFFIX}" ]; then
    FILENAME="evebox-devel-${ARCH}.deb"
else
    FILENAME="evebox-${VERSION}-${ARCH}.deb"
fi

fpm --verbose -t deb -n evebox -s dir --epoch 1 \
    -a "${ARCH}" \
    -p "dist/${FILENAME}" \
    -v "${VERSION}" \
    --force \
    --after-install=./packaging/debian/after-install.sh \
    --after-upgrade=./packaging/debian/after-upgrade.sh \
    --config-files /etc/evebox/evebox.yaml \
    "${BIN}"=/usr/bin/evebox \
    ./packaging/debian/evebox.service=/lib/systemd/system/evebox.service \
    ./packaging/debian/evebox-agent.service=/lib/systemd/system/evebox-agent.service \
    ./packaging/evebox.yaml=/etc/evebox/

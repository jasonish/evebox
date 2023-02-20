#! /bin/sh

set -e

CARGO_VERSION=$(cat Cargo.toml | awk '/^version/ { gsub(/"/, "", $3); print $3 }')
VERSION=$(echo ${CARGO_VERSION} | sed 's/\(.*\)\-.*/\1/')
VERSION_SUFFIX=$(echo ${CARGO_VERSION} | sed -n 's/.*-\(.*\)/\1/p')
DATE=$(date +%s)

ARGS=()

case "${1}" in
    "x86_64"|"amd64")
	ARGS+=("-a" "amd64")
	ARCH="amd64"
	BIN="./dist/evebox-latest-linux-x64/evebox"
	;;
    "arm64")
	ARGS+=("-a" "arm64")
	ARCH="arm64"
	BIN="./dist/evebox-latest-linux-arm64/evebox"
	;;
    "arm")
	ARGS+=("-a" "armhf")
	ARCH="armhf"
	BIN="./dist/evebox-latest-linux-arm/evebox"
	;;
    *)
	echo "error: invalid ARCH"
	exit 1
esac

if [ "${VERSION_SUFFIX}" ]; then
    VERSION="${VERSION}~${VERSION_SUFFIX}${DATE}"
fi

if [ "${VERSION_SUFFIX}" ]; then
    FILENAME="evebox-latest-${ARCH}.deb"
else
    FILENAME="$evebox-${VERSION}-${ARCH}.deb"
fi

fpm -t deb -n evebox -s dir --epoch 1 "${ARGS[@]}" \
    -a "${ARCH}" \
    -p "dist/${FILENAME}" -v "${VERSION}" \
    --force \
    --after-install=deb/after-install.sh \
    --after-upgrade=deb/after-upgrade.sh \
    --deb-no-default-config-files \
    --config-files /etc/default/evebox \
    "${BIN}"=/usr/bin/evebox \
    ./examples/evebox.yaml=/etc/evebox/evebox.yaml.example \
    ./examples/agent.yaml=/etc/evebox/agent.yaml.example \
    ./deb/evebox.default=/etc/default/evebox \
    ./deb/evebox.service=/lib/systemd/system/evebox.service \
    ./deb/evebox-agent.service=/lib/systemd/system/evebox-agent.service

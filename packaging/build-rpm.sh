#! /bin/sh

set -e
set -x

CARGO_VERSION=$(cat Cargo.toml | awk '/^version/ { gsub(/"/, "", $3); print $3 }')
VERSION=$(echo ${CARGO_VERSION} | sed 's/\(.*\)\-.*/\1/')
VERSION_SUFFIX=$(echo ${CARGO_VERSION} | sed -n 's/.*-\(.*\)/\1/p')
DATE=$(date +%s)

case "${1}" in
    "x86_64"|"amd64")
	ARCH="x86_64"
	BIN="./dist/evebox-latest-linux-x64/evebox"
	;;
    *)
	echo "error: invalid ARCH"
	exit 1
esac

if [ "${VERSION_SUFFIX}" ]; then
    RPM_ITERATION="0.${VERSION_SUFFIX}${DATE}"
    OUTPUT="evebox-latest-${ARCH}.rpm"
else
    RPM_ITERATION="1"
    OUTPUT=""
fi

fpm --verbose -t rpm -n evebox -s dir --epoch 1 \
    -a "${ARCH}" \
    -v "${VERSION}" \
    -p "./dist/${OUTPUT}" \
    --force \
    --iteration "${RPM_ITERATION}" \
    --before-install=./packaging/rpm/before-install.sh \
    --after-upgrade=./packaging/rpm/after-upgrade.sh \
    --config-files /etc/evebox/evebox.yaml \
    --rpm-attr 0644,root,root:/lib/systemd/system/evebox.service \
    --rpm-attr 0644,root,root:/lib/systemd/system/evebox-agent.service \
    --rpm-attr 0755,root,root:/usr/bin/evebox \
    ${BIN}=/usr/bin/evebox \
    ./packaging/rpm/evebox.service=/lib/systemd/system/evebox.service \
    ./packaging/rpm/evebox-agent.service=/lib/systemd/system/evebox-agent.service \
    ./packaging/evebox.yaml=/etc/evebox/evebox.yaml

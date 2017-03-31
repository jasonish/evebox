#! /bin/bash

MINGW_HOME=/cygdrive/c/mingw-w64/x86_64-6.3.0-posix-seh-rt_v5-rev1/mingw64
export PATH=${MINGW_HOME}/bin:${GOPATH}/bin:$PATH

echo "Gcc version: $(gcc --version)"
echo "Go version: $(go version)"
echo "Node version: $(node --version)"

# Install glide then use it to install the Go dependencies.
if [ -e ./windows-amd64/glide.exe ]; then
    echo "Found cached glide.exe."
else
    echo "Downloading glide."
    GLIDE_BASE_URL=https://github.com/Masterminds/glide/releases/download
    GLIDE_VERSION=0.12.3
    GLIDE_FILENAME=glide-v${GLIDE_VERSION}-windows-amd64.zip
    curl -OL ${GLIDE_BASE_URL}/v${GLIDE_VERSION}/${GLIDE_FILENAME}
    unzip ${GLIDE_FILENAME}
    rm -f ${GLIDE_FILENAME}
fi

if [ -e ./vendor ]; then
    echo "Found cached ./vendor."
else
    echo "Running glide install."
    ./windows-amd64/glide.exe install
fi

# go-bindata isn't handle by glide as we need its executable.
go get github.com/jteeuwen/go-bindata/...

if [ -e ./webapp/node_modules ]; then
    echo "Found cached node_modules."
else
    echo "Install JavaScript dependencies."
    NO_PROGRESS=1 make -C webapp install-deps
fi

BUILD_DATE_ISO=$(TZ=UTC date +%Y%m%d%H%M%S)
BUILD_DATE_ISO=${BUILD_DATE_ISO} WITH_SQLITE=1 GOOS=windows GOARCH=amd64 \
	      make dist

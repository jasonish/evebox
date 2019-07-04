#! /bin/bash

set -e
set -x

OSXCROSS_SDK_VERSION="10.11"

cd /opt
git clone https://github.com/tpoechtrager/osxcross.git
cd osxcross
curl -L -o \
     ./tarballs/MacOSX${OSXCROSS_SDK_VERSION}.sdk.tar.xz \
     https://s3.amazonaws.com/andrew-osx-sdks/MacOSX${OSXCROSS_SDK_VERSION}.sdk.tar.xz

sed -i -e 's|-march=native||g' ./build_clang.sh ./wrapper/build.sh

printf "\n" | PORTABLE=true bash -x ./build.sh

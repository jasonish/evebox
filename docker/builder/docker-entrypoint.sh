#! /bin/bash

set -e
set -x

if [ -e /opt/rh/rh-ruby23/enable ]; then
    . /opt/rh/rh-ruby23/enable
fi

if [ -e /opt/osxcross ]; then
    export PATH=/opt/osxcross/target/bin:$PATH
fi

exec $@

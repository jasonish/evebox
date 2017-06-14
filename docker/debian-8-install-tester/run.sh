#! /bin/sh

if ! test -e ./dist/evebox*.deb; then
    echo "error: this script must be run from the top evebox directory with"
    echo "   a debian package built."
    exit 1
fi

name=$(basename $(dirname $0))
tag="evebox/${name}"

docker build -t ${tag} -f docker/${name}/Dockerfile .
docker run --rm -it ${tag}

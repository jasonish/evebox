#! /bin/sh

name=$(basename $(dirname $0))
tag="evebox/${name}"

docker build -t ${tag} -f docker/${name}/Dockerfile .
docker run --rm -it ${tag}

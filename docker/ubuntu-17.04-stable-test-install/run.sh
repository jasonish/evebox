#! /bin/sh

tag="evebox/$(basename $(pwd))"
cd $(dirname $0)
docker build -t ${tag} .
docker run --rm ${tag}

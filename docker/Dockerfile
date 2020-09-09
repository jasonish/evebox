ARG BASE
FROM $BASE

ARG SRC="SRC_--build-arg_must_be_set"
COPY $SRC /bin/evebox

ENV EVEBOX_HTTP_HOST=0.0.0.0

COPY /docker/docker-entrypoint.sh /docker-entrypoint.sh
ENTRYPOINT ["/docker-entrypoint.sh"]
CMD ["/bin/evebox"]

ARG BASE
FROM $BASE

ARG SRC="SRC_--build-arg_must_be_set"
COPY $SRC /bin/evebox

ENV EVEBOX_HTTP_HOST=0.0.0.0
ENV TZ=UTC

ENV EVEBOX_DATA_DIRECTORY=/var/lib/evebox
RUN mkdir ${EVEBOX_DATA_DIRECTORY}
VOLUME ${EVEBOX_DATA_DIRECTORY}

COPY /docker/docker-entrypoint.sh /docker-entrypoint.sh
ENTRYPOINT ["/docker-entrypoint.sh"]
CMD ["/bin/evebox"]

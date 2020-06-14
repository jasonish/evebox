FROM scratch

ARG SRC="SRC_--build-arg_must_be_set"
COPY $SRC /evebox

ENV EVEBOX_HTTP_HOST=0.0.0.0
CMD ["/evebox"]

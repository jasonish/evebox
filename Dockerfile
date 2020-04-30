# Stage 1 - Build.
FROM fedora:32

RUN dnf -y install \
        curl \
        findutils \
        git \
        gcc \
        musl-gcc \
        make \
        zip


ENV N_V 10.14.2
RUN cd /usr/local && \
  curl -o - -L https://nodejs.org/dist/v${N_V}/node-v${N_V}-linux-x64.tar.gz | \
       tar zxf - --strip-components=1

RUN curl https://sh.rustup.rs -sSf | sh -s -- --default-toolchain stable -y
ENV PATH /root/.cargo/bin:$PATH
RUN rustup target add x86_64-unknown-linux-musl

WORKDIR /src
COPY / .
RUN RELEASE="--release" TARGET="x86_64-unknown-linux-musl" make

# State 2 - Copy in binary to clean container.
FROM alpine:latest
COPY --from=0 /src/target/x86_64-unknown-linux-musl/release/evebox /bin/evebox
RUN /bin/evebox version
COPY /docker/docker-entrypoint.sh /docker-entrypoint.sh
ENTRYPOINT ["/docker-entrypoint.sh"]
CMD ["/bin/evebox", "server"]

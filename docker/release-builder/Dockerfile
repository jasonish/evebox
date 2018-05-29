# Dockerfile for building EveBox releases.

FROM centos:7

RUN yum -y install epel-release
RUN yum -y install \
    tar \
    curl \
    which \
    zip \
    git \
    make \
    gcc \
    gcc-c++ \
    rpm-build \
    autoconf \
    automake \
    libtool

ENV NODE_VERSION 8.11.2
RUN mkdir /usr/local/node && \
    cd /usr/local/node && \
    curl -L https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz | \
    tar Jxf - --strip-components=1

ENV GO_VERSION 1.10.2
RUN cd /usr/local && \
    curl -L https://storage.googleapis.com/golang/go${GO_VERSION}.linux-amd64.tar.gz | \
    tar zxf -

# Install Ruby from SCL, and fpm.
RUN yum -y install \
    centos-release-scl \
    yum-utils && \
    yum-config-manager --enable rhel-server-rhscl-7-rpms && \
    yum -y install rh-ruby23 rh-ruby23-ruby-devel
RUN . /opt/rh/rh-ruby23/enable && gem install --bindir=/usr/local/bin fpm

ENV GOPATH /go
ENV PATH $PATH:/usr/local/node/bin:$GOPATH/bin:/usr/local/go/bin
ENV PROJECT_DIR $GOPATH/src/github.com/jasonish/evebox

WORKDIR $PROJECT_DIR

COPY /docker/release-builder/docker-entrypoint.sh /
COPY / $PROJECT_DIR

ENTRYPOINT /docker-entrypoint.sh

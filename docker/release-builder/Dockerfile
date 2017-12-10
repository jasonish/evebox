# Dockerfile for building EveBox releases.
#
# The idea with using CentOS 6 is so we link against an older libc
# that is likely to run everywhere (though this idea may be
# flawed). But this is Go and it doesn't use libc? Well, SQLite is
# used which means we're using cgo on Linux.

FROM centos:6

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
    rpm-build

ENV NODE_VERSION 8.9.3
RUN mkdir /usr/local/node && \
    cd /usr/local/node && \
    curl -L https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz | \
    tar Jxf - --strip-components=1

ENV GO_VERSION 1.9.2
RUN cd /usr/local && \
    curl -L https://storage.googleapis.com/golang/go${GO_VERSION}.linux-amd64.tar.gz | \
    tar zxf -

# Install Ruby from SCL, and fpm.
RUN yum -y install \
    centos-release-scl \
    yum-utils && \
    yum-config-manager --enable rhel-server-rhscl-6-rpms && \
    yum -y install rh-ruby22 rh-ruby22-ruby-devel
RUN . /opt/rh/rh-ruby22/enable && gem install --bindir=/usr/local/bin fpm

ENV GOPATH /go
ENV PATH $PATH:/usr/local/node/bin:$GOPATH/bin:/usr/local/go/bin
ENV PROJECT_DIR $GOPATH/src/github.com/jasonish/evebox

# Install glide. Go get has been known to get broken versions but
# normally I'd avoid install software this way.
RUN mkdir -p $GOPATH/bin && \
    curl https://glide.sh/get | sh

WORKDIR $PROJECT_DIR

COPY /docker/release-builder/docker-entrypoint.sh /
COPY / $PROJECT_DIR

ENTRYPOINT /docker-entrypoint.sh

FROM centos:7

RUN yum -y install \
    tar \
    curl \
    which \
    zip \
    git \
    make \
    gem \
    gcc \
    gcc-c++ \
    ruby-devel \
    rpm-build && \
    gem install fpm

ENV NODE_VERSION 6.10.1
RUN mkdir /usr/local/node && \
    cd /usr/local/node && \
    curl -L https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz | \
    tar Jxf - --strip-components=1

ENV GO_VERSION 1.8
RUN cd /usr/local && \
    curl -L https://storage.googleapis.com/golang/go${GO_VERSION}.linux-amd64.tar.gz | \
    tar zxf -

ENV GOPATH /go
ENV PATH $PATH:/usr/local/node/bin:$GOPATH/bin:/usr/local/go/bin

# Install glide. Go get has been known to get broken versions but
# normally I'd avoid install software this way.
RUN mkdir -p $GOPATH/bin && \
    curl https://glide.sh/get | sh

RUN go get github.com/cespare/reflex && \
    go get github.com/jteeuwen/go-bindata/...

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

ENV NODE_VERSION 4.5.0
RUN mkdir /usr/local/node && \
    cd /usr/local/node && \
    curl -L https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz | \
    	 tar Jxvf - --strip-components=1

RUN cd /usr/local && \
    curl -L https://storage.googleapis.com/golang/go1.7.linux-amd64.tar.gz | \
    tar zxvf -

ENV GOPATH /go
ENV PATH $PATH:/usr/local/node/bin:$GOPATH/bin:/usr/local/go/bin

# Install glide. Go get has been known to get broken versions.
RUN mkdir -p $GOPATH/bin && \
    curl https://glide.sh/get | sh

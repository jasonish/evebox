FROM fedora:rawhide

RUN dnf -y install \
    golang \
    nodejs \
    git \
    make \
    zip \
    tar \
    findutils

ENV GOPATH /go
ENV PATH=$GOPATH/bin:$PATH

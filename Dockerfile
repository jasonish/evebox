FROM fedora:23

RUN dnf -y install \
    golang \
    git \
    make \
    zip \
    tar

RUN dnf -y install tar && \
    curl -O https://nodejs.org/dist/v4.2.1/node-v4.2.1-linux-x64.tar.gz && \
    cd /usr/local && \
    tar zxvf /node-v4.2.1-linux-x64.tar.gz --strip-components=1

ENV GOPATH /gopath
ENV PATH=$GOPATH/bin:$PATH


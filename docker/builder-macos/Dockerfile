FROM fedora:29

RUN dnf -y install \
    	make \
	git \
	gcc \
	gcc-c++ \
	zip \
	sudo \
	rpm-build \
	which \
	mingw64-gcc \
	clang \
	patch \
	cmake \
	libxml2-devel \
	openssl-devel

ENV GO_V 1.11.4
RUN cd /usr/local && \
    curl -o - -L https://dl.google.com/go/go${GO_V}.linux-amd64.tar.gz | \
        tar zxf -

ENV N_V 10.14.2
RUN cd /usr/local && \
  curl -o - -L https://nodejs.org/dist/v${N_V}/node-v${N_V}-linux-x64.tar.gz | \
       tar zxf - --strip-components=1

ENV PATH /usr/local/go/bin:$PATH

RUN groupadd --gid 5000 builder
RUN useradd --uid 5000 --gid 5000 --password "" --groups wheel builder
RUN echo "builder ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/builder

COPY /docker/builder-macos/install-osxcross.sh /
RUN /install-osxcross.sh

COPY /docker/builder-macos/docker-entrypoint.sh /
ENTRYPOINT ["/docker-entrypoint.sh"]


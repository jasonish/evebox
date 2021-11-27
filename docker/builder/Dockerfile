FROM centos:7

RUN yum -y install epel-release && \
    yum -y install \
	clang \
	gcc \
	gcc-c++ \
	git \
    	make \
	mingw64-gcc \
        openssl-devel \
	patch \
	rpm-build \
	sudo \
	which \
	zip

ENV N_V 16.13.0
RUN cd /usr/local && \
  curl -o - -L https://nodejs.org/dist/v${N_V}/node-v${N_V}-linux-x64.tar.gz | \
       tar zxf - --strip-components=1

ARG REAL_UID=1000
ARG REAL_GID=1000
RUN groupadd --gid ${REAL_GID} builder
RUN useradd --uid ${REAL_UID} --gid ${REAL_GID} --password "" --groups wheel builder
RUN echo "builder ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/builder

USER builder

COPY /docker/builder/docker-entrypoint.sh /
ENTRYPOINT ["/docker-entrypoint.sh"]

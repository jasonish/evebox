FROM centos:7

RUN yum -y install \
    	make \
	git \
	gcc \
	gcc-c++ \
	zip \
	sudo \
	rpm-build

# Install Ruby from SCL, and fpm.
RUN yum -y install \
    centos-release-scl \
    yum-utils && \
    yum-config-manager --enable rhel-server-rhscl-7-rpms && \
    yum -y install rh-ruby23 rh-ruby23-ruby-devel
RUN . /opt/rh/rh-ruby23/enable && gem install --bindir=/usr/local/bin fpm

ENV GO_V 1.11.4
RUN cd /usr/local && \
    curl -o - -L https://dl.google.com/go/go${GO_V}.linux-amd64.tar.gz | \
        tar zxf -

ENV N_V 10.14.2
RUN cd /usr/local && \
  curl -o - -L https://nodejs.org/dist/v${N_V}/node-v${N_V}-linux-x64.tar.gz | \
       tar zxf - --strip-components=1

ENV PATH /usr/local/go/bin:$PATH

RUN useradd builder
RUN echo "builder ALL=(ALL) NOPASSWD: ALL" > /etc/sudoers.d/builder

COPY /docker/builder/builder-entrypoint.sh /

ENTRYPOINT ["/builder-entrypoint.sh"]

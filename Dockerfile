# Stage 1 - Build.
FROM centos:7

RUN cd /usr/local && \
    curl -o - -L https://dl.google.com/go/go1.11.4.linux-amd64.tar.gz | \
        tar zxf -

ENV N_V 10.14.2
RUN cd /usr/local && \
  curl -o - -L https://nodejs.org/dist/v${N_V}/node-v${N_V}-linux-x64.tar.gz | \
       tar zxf - --strip-components=1

ENV PATH /usr/local/go/bin:$PATH

RUN yum -y install \
    	make \
	git \
	gcc

WORKDIR /src
COPY / .
RUN make install-deps && make

# State 2 - Copy in binary to clean container.
FROM centos:7
COPY --from=0 /src/evebox /bin/evebox
COPY /docker/docker-entrypoint.sh /docker-entrypoint.sh
ENTRYPOINT ["/docker-entrypoint.sh"]
CMD ["/bin/evebox", "server"]

FROM alpine:latest
MAINTAINER Jason Ish <ish@unx.ca>

# Attempt multiple times to download evebox from bintray.
#
# This build is triggered from Travis-CI just after deploying to Bintray which
# sometimes causes a period of time where the URL fails with a 401 error. To get
# around this we attempt the download multiple times.
RUN cd /tmp && \
    for i in `seq 1 10`; do \
    	if wget http://dl.bintray.com/jasonish/evebox-development/evebox-latest-linux-amd64.zip; \
	then \
	     break; \
	else \
	     sleep 10; \
	fi \
    done && \
    unzip evebox-latest-linux-amd64.zip && \
    mkdir -p /usr/local/bin && \
    cp -a evebox-*-linux-amd64/evebox /usr/local/bin/ && \
    rm -rf evebox* && \
    /usr/local/bin/evebox version

EXPOSE 5636

COPY /docker-entrypoint.sh /

ENTRYPOINT ["/docker-entrypoint.sh"]
CMD ["evebox", "server"]

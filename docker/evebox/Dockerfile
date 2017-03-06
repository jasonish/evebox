FROM fedora:25
MAINTAINER Jason Ish <ish@unx.ca>

RUN dnf -y install unzip

# Attempt multiple times to download evebox from bintray.
#
# This build is triggered from Travis-CI just after deploying to Bintray which
# sometimes causes a period of time where the URL fails with a 401 error. To get
# around this we attempt the download multiple times.
ENV DISTNAME evebox-latest-linux-x64
ENV FILENAME ${DISTNAME}.zip
ENV URL http://dl.bintray.com/jasonish/evebox-development/${FILENAME}
RUN cd /tmp && \
    for i in `seq 1 10`; do \
    	if curl -OL ${URL}; \
	then \
	     break; \
	else \
	     sleep 10; \
	fi \
    done && \
    unzip ${FILENAME} && \
    mkdir -p /usr/local/bin && \
    cp -a ${DISTNAME}/evebox /usr/local/bin/ && \
    rm -rf evebox* && \
    /usr/local/bin/evebox version

EXPOSE 5636

COPY /docker-entrypoint.sh /

ENTRYPOINT ["/docker-entrypoint.sh"]
CMD ["evebox", "server"]

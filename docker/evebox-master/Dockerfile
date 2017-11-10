FROM fedora:26
MAINTAINER Jason Ish <ish@unx.ca>

RUN rpm -Uvh https://evebox.org/files/rpm/development/evebox-development-release.noarch.rpm && \
    dnf install --disablerepo=* --enablerepo=evebox-development -y evebox && \
    /usr/bin/evebox version

EXPOSE 5636

COPY /docker-entrypoint.sh /

ENTRYPOINT ["/docker-entrypoint.sh"]
CMD ["evebox", "server"]

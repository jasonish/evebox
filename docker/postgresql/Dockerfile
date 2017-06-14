FROM centos:7
MAINTAINER <evebox@evebox.org>

RUN rpm -i https://download.postgresql.org/pub/repos/yum/9.6/redhat/rhel-7-x86_64/pgdg-centos96-9.6-3.noarch.rpm && \
    yum -y install \
    	which \
    	postgresql96-server \
	postgresql96-contrib

COPY /docker-entrypoint.sh /docker-entrypoint.sh
EXPOSE 5432
ENTRYPOINT /docker-entrypoint.sh

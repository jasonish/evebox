FROM centos:7

RUN cd /etc/yum.repos.d && \
    curl -OL https://copr.fedorainfracloud.org/coprs/jasonish/suricata-stable/repo/epel-7/jasonish-suricata-stable-epel-7.repo && \
    yum -y install epel-release && \
    yum -y install suricata && \
    suricata -V

RUN yum -y install \
    	   python-pip \
	   python-yaml && \
    pip install https://github.com/OISF/suricata-update/archive/master.zip && \
    suricata-update --version

RUN yum -y install \
    	   iproute

# Seed the rules.
RUN suricata-update --etopen --no-test --no-reload

COPY /suricata-demo.yaml /etc/suricata/suricata-demo.yaml

COPY /docker-entrypoint.sh /docker-entrypoint.sh

VOLUME /var/log/suricata

ENTRYPOINT ["/docker-entrypoint.sh"]

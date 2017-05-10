# /bin/sh

set -e

apt-get -y update
apt-get -y install wget
wget -qO - https://evebox.org/files/GPG-KEY-evebox | apt-key add -
echo "deb http://files.evebox.org/evebox/debian stable main" | \
    tee /etc/apt/sources.list.d/evebox.list
apt-get -y update
apt-get -y install evebox
evebox version

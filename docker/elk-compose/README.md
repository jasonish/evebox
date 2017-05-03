Small Docker Compose Setup for Logstash and Elastic Search

## Usage

First make sure you have Suricata logging to
/var/log/suricata/eve.json.

Then, from this directory run:
```
docker-compose up
```

Then run _EveBox_, pointing it at Elastic Search on localhost:
```
./evebox -v -e http://localhost:9200 -i logstash

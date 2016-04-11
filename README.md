[![Build Status](https://travis-ci.org/jasonish/evebox.svg?branch=master)](https://travis-ci.org/jasonish/evebox)
[![Download](https://api.bintray.com/packages/jasonish/evebox/latest/images/download.svg) ](https://bintray.com/jasonish/evebox/latest/latest/view/files)

# EveBox

EveBox is a web based Suricata "eve" event viewer for Elastic Search.

## Requirements

- Suricata, Logstash and Elastic Search (Elastic Search 2.0 or newer).
- A modern browser.

## Installation.

### Option 1 - Built In Backend (Recommended)

Download a package and run the evebox application. This allows you to
run with a default Elastic Search install without having to enable
CORS. It also doesn't require setting up a web server to server Evebox
like previously required.

Example:

    ./evebox -e http://localhost:9200

Then visit http://localhost:5636 with your browser.

Up to date builds can be found here:
https://bintray.com/jasonish/evebox/latest/latest/view/files

This should not require any modification to your Elastic Search
configuration. Unlike previous versions of Evebox, you do not need to
enable dynamic scripting and CORS.

### Option 2 - Docker (Also recommended)

Example:

```
docker run -it -p 5636:5636 jasonish/evebox -e http://elasticsearch:9200
```

replacing your __http://elasticsearch:9200__ with that of your Elastic
Search URL. You most likely do not want to use localhost here as that
will be the localhost of the container, not of the host.

OR if you want to link to an already running Elastic Search container:

```
docker run -it -p 5636:5636 --link elasticsearch jasonish/evebox
```

Then visit http://localhost:5636 with your browser.

This should not require any modification to your Elastic Search
configuration. Unlike previous versions of Evebox, you do not need to
enable dynamic scripting and CORS.

## Building EveBox

EveBox consists of a JavaScript frontend, and a very minimal backend
written in Go.

Frontend requirements:

* Node.js v4.2.1 or newer.

Backend requirements:

* A working Go 1.5 installation and GOPATH.

## Run in Development Mode

First you need to install dependencies and build the software:

make install-deps all

Then you can run

EVEBOX_ELASTICSEARCH_URL=http://localhost:9200 make dev-server

Where the EVEBOX_ELASTICSEARCH_URL is pointing to your Elastic Search
server.

## License

BSD.

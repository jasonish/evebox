[![Build Status](https://travis-ci.org/jasonish/evebox.svg?branch=master)](https://travis-ci.org/jasonish/evebox)
[![Download](https://api.bintray.com/packages/jasonish/evebox-development/evebox/images/download.svg) ](https://bintray.com/jasonish/evebox-development/evebox/_latestVersion#files)

# EveBox

EveBox is a web based Suricata "eve" event viewer for Elastic Search.

![EveBox](https://codemonkey.net/evebox/screens/escalated.png)

## Requirements

- Suricata, Logstash and Elastic Search (Elastic Search 2.0 or newer).
- A modern browser.

## Installation.

Download a package and run the evebox application.

Example:

    ./evebox -e http://localhost:9200

Then visit http://localhost:5636 with your browser.

Up to date builds can be found here:
https://bintray.com/jasonish/evebox-development/evebox/_latestVersion#files

This should not require any modification to your Elastic Search
configuration. Unlike previous versions of Evebox, you do not need to
enable dynamic scripting and CORS.

## Docker

If you wish to install EveBox with Docker an up to date image is
hosted on Docker hub.

Example:

```
docker pull jasonish/evebox
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
written in Go. To build Evebox the following requirements must first
be satisfied:

* Node.js v6.5.0 or newer installed.
* A working Go 1.7 installation and GOPATH.

First checkout Evebox into your GOPATH, for example:

```
git clone https://github.com/jasonish/evebox.git \
    $GOPATH/src/github.com/jasonish/evebox
```

If this is the first build the npm and Go dependencies must be
installed, this can be done with:
```
make install-deps
```

```install-deps``` will also upgrade any dependencies, so its a good idea
to re-run after git pulls.

Then to build the binary:
```
make
```

Or to build a release:
```
make release
```

If you don't want to bother with the required development tools, but do have
Docker installed, you can build a release with the following command:
```
make release-with-docker`
```

## Run in Development Mode

ELASTICSEARCH_URL=http://localhost:9200 make dev-server

Where the ELASTICSEARCH_URL is pointing to your Elastic Search server.

## License

BSD.

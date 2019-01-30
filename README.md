# EveBox [![Documentation Status](https://readthedocs.org/projects/evebox/badge/?version=latest)](https://evebox.readthedocs.io/en/latest/?badge=latest) [![Build Status](https://travis-ci.org/jasonish/evebox.svg?branch=master)](https://travis-ci.org/jasonish/evebox)

EveBox is a web based Suricata "eve" event viewer for Elastic Search.

![EveBox](https://evebox.org/screens/inbox.png)

## Features

- A web based event viewer with an "Inbox" approach to alert
  management.
- Event search.
- An agent for sending Suricata events to the EveBox server (but you
  can use Filebeat/Logstash instead).
- Embedded SQLite for self-contained installations.

## Requirements

- Suricata - to generate alerts and events.

And one of...

- An existing ELK (version 5 or greater) setup already handling
  Suricata events.
- Just Elastic Search as an event store.
- Nothing - EveBox can use an embedded SQLite database suitable for
  lower load installations (note: not all features supported yet).
- A modern web browser.

## Installation.

Download a package and run the evebox application against your
existing Elastic Search server.

Example:

    ./evebox -e http://localhost:9200

Then visit http://localhost:5636 with your browser.

The latest release builds can be found at
https://evebox.org/files/release/latest/.

The latest development builds (from git master) can be found at
https://evebox.org/files/development/

A [RPM](https://github.com/jasonish/evebox/wiki/EveBox-RPM-Repository)
and
[Debian](https://github.com/jasonish/evebox/wiki/EveBox-Debian-Repository) package
repository are also available.

### SELKS

EveBox is also included
in [SELKS](https://www.stamus-networks.com/open-source/) which
provides Suricata and an ELK stack configured and ready to go.

### Docker

If you wish to install EveBox with Docker an up to date image is
hosted on Docker hub.

Example:

```
docker pull jasonish/evebox:latest
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

## Usage

EveBox runs as a server exposing a web interface on port 5636 by
default.

### With an Existing Elastic Search Server With Events

The basic mode where `eve` events are being sent to Elastic Search
with Logstash and or Filebeat.

```
evebox server -e http://elasticsearch:9200
```

### With the Embedded SQLite Database

This is useful if you don't have Elastic Search and running EveBox on
the same machine as Suricata. It uses an embedded SQLite database for
events and is suitable for ligher loads. Currently SQLite does not
support reporting.

```
evebox server --datastore sqlite --input /var/log/suricata/eve.json
```

More documentation can be found at http://evebox.readthedocs.io/en/latest/.

## Building EveBox

EveBox consists of a JavaScript frontend, and a very minimal backend
written in Go. To build Evebox the following requirements must first
be satisfied:

* Node.js v10.13.0 or newer installed.
* Go 1.11.1 or new installed.

First checkout EveBox. As EveBox uses Go 1.11 modules, do not check it
out into your GOPATH.

For example:

```
git clone https://github.com/jasonish/evebox.git ~/project/evebox
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

Or to build a release package:
```
make dist
```

If you don't want to bother with the required development tools, but do have
Docker installed, you can build a release with the following command:
```
./docker.sh release
```

## Run in Development Mode

```
./dev.sh -e http://elasticsearch:9200
```

to run in development mode using an Elastic Search datastore at
http://elasticsearch:9200.

The connect your browser to http://localhost:4200. Note this port is
different than the EveBox port, as the Angular CLI/Webpack development
server is used to serve up the web application with backend requests
being proxied to the Go application.

In development mode changes to Go files will trigger a
recompile/restart, and changes to the web app will trigger a recompile
of the javascript and a browser refresh.

## A Note on Authentication

While the latest development versions of EveBox support
authentication, TLS support is not included. Therefore it is advised
to run EveBox behind a reverse proxy that terminals TLS/SSL.

## Change Log

See https://github.com/jasonish/evebox/blob/master/CHANGELOG.md .

## License

BSD.

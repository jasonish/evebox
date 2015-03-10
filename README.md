# EveBox

EveBox is a Suricata "eve" event viewer for Elastic Search.

# Requirements

- Suricata, Logstash and Elastic Search (Elastic Search v1.3.0 or newer).
- A webserver.  EveBox consists of static files only.  Apache, Nginx
  or any other static file webserver will do.
- A modern browser.

# Installation.

Clone this repo and serve up the app directory.

Or...

1. Get the latest release from https://github.com/jasonish/evebox/releases.
2. Extract evebox-<version>.zip to your webserver.
3. Optionally copy sample-config.js to config.js and modify.  The
   Elastic Search settings can also be set from the user interface,
   but will need to be done from each browser.

# Elastic Search Setup

## CORS

As of Elastic Search 1.4, CORS is disabled by default.  It can be
enabled by setting the following in your elasticsearch.yml:

```
http.cors.enabled: true
```

See the Elastic Search manual for further restrictions on CORS.

## Dynamic Scriptiing

Dynamic scripting is also required for EveBox to function, this can be
enabled in a quick and dirty fashion by adding the following to your
elasticsearch.yml:

```
script.disable_dynamic: false
```

Note that this can expose your Elastic Search to some security issues,
so it is recommended that you have taken proper steps to secure your
Elastic Search behind a proxy.

# Suricata / Logstash Setup

EveBox currently works around the concept of an inbox.  That is, events
go into the inbox until they are archive (acknowledged) or deleted.
This is done by adding the "inbox" tag to events with Logstash.

Currently EveBox really only works with event_type alert, so to have
all new alerts show up in the inbox a Logstash filter like the
following can be added:

    filter {

      # Add the "inbox" tag to all incoming alerts.
      if [event_type] == "alert" {
	    mutate {
		  add_tag => ["inbox"]
	    }
	  }

    }

# Usage

EveBox is built around keyboard shorcuts.  Hit the "Help" link or
simply type "?" to get a list of keyboard shortcuts.  If you are
familiar with GMail keyboard shortcuts it should feel familiar very
quickly.

# TODOs
- A darker theme.
- Arbitrary tagging.
- A backend might be needed to do such things as large bulk tagging.

# License

BSD.

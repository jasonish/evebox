# Change Log

## 0.17.0 - unreleased

- Move to SolidJS for frontend development.
- New special query string keywords:
  - @ip: match src_ip or dest_ip
  - @earliest:TIMESTAMP
  - @latest:TIMESTAMP
- Feature parity between SQLite and Elasticsearch. This means that
  some reports were removed, but should come back for both SQLite and
  Elasticsearch.
- [sqlite] Enable event retention by default to a value of 7 days. If
  an SQLite database becomes too large, it can be hard to trim back
  down to a usable size without significant downtime.
- Start on a new overview report.
- Fix issue where alert report graph didn't refresh over time change:
  https://github.com/jasonish/evebox/issues/247

## 0.16.0 - 2022-11-12

- [server] Fix authentication:
  https://github.com/jasonish/evebox/issues/227,
  https://github.com/jasonish/evebox/issues/230
- [server] Auto archive: https://github.com/jasonish/evebox/issues/52
- [webapp] Update to Bootstrap 5
- [webapp] Update to Angular 14
- [sqlite] Typo when opening sqlite database:
  https://github.com/jasonish/evebox/issues/226
- Many cleanups from 0.15.0

## 0.15.0 - 2022-02-27

- [sqlite] Remove full text search engine. It provided little benefit on search
  and was very expensive to add events to.
- Add a stats view.
- [webapp] Update to Angular 13.
- [server] Move from Warp to Axum.
- [webapp] Remove Brace editor for pretty printing of JSON and replace with
  a JSON pretty printer module.
- [elastic] Fixes to Elastic field name mappings that should address issues
  with ECS. Most things seem to work.

## 0.14.0 - 2021-06-16

- Relicense under MIT, oops.
- Server: Wait for Elasticsearch to be ready:
  https://github.com/jasonish/evebox/issues/170
- Fix add users command to take parameters from command line as
  documented: https://github.com/jasonish/evebox/issues/173
- Rule parser: Fix stripping of quotes:
  https://github.com/jasonish/evebox/issues/177

## 0.13.1 - 2021-04-09

- Fix http request logging, and logging the remote IP address when behind a
  reverse proxy: https://github.com/jasonish/evebox/issues/163
- Fix client side authentication issue:
  https://github.com/jasonish/evebox/issues/160
- Fix initialization of SQLite events database:
  https://github.com/jasonish/evebox/issues/166

## 0.13.0 - 2021-03-18

### Fixes
- Flow report fixes.
- Netflow report fixes.
- Capitalization of app_proto's in web.
- When converting a packet to pcap, use the linktype from the packet info if
  available. If not available use ethernet. Fixes the case where the packet is
  from nfqueue, where its DLT_RAW.
- Unfocus time range selector after a new range is selected allowing keyboard
  shortcuts to work again without having to click somewhere in the page.
- Fix issue where the input section in the configuration file was
  being used even if enabled was set to false. This only happened when
  using a configuration file with an input section:
  https://github.com/jasonish/evebox/issues/159

### Changes
- Server: Allow wildcard in input filename to allow the usage of threaded eve
  output. For example: /var/log/suricata/eve.*.json.
- Agent: Allow multiple input paths to be specified.
- New keyboard shortcut, '\\' to open time range selector.

### Features
- New DHCP report that attempts to give you a picture of the devices that have 
  been assigned an IP(v4) address over the requested period of time.

## 0.12.0 - 2020-09-25

### Changes
- Server rewritten in Rust. Ideally this should not be noticed.
- Stop tagging events with "archived" and "escalated", and only use
  "evebox.archived" and "evebox.escalated". This should not be noticed
  as EveBox has been using both tags for a very long.
- The Docker image is now based on Alpine Linux. Scratch could be
  used, but it would break compatibility with previous images.
- Agent: The baheaviour of using the log filename suffixed with ".bookmark
  has been removed. The agent will prefer to use the configured bookmark
  directory (aka data-directory) instead, or if not set, the current
  directory where EveBox is being run from. However, if these deprecated
  bookmark filenames exist (like after an upgrade), they will continue
  to be used.
- The command "esimport" has been renamed to "elastic-import".

### Fixes
- Fix the index_pattern when adding a template to Elasticsearch with a
  non logstash index.
- Fix disabling of certificate checks for connecting to an Elasticsearch
  server with a self-signed certificate.
  https://github.com/jasonish/evebox/issues/144

### Breaking Changes
- License: AGPL
- LetsEncrypt support has been removed.

### Known Issues
- When using a self-signed certificate, the hostname being connected
  to must match the hostname in the certificate.

## 0.11.0 - 2020-03-26

### Enhancements
- Handle Filebeat overriding the "host" field with its own object by
  normalizing the sensor name before rendering. If Filebeat is used,
  the Suricata provided sensor name is lost, so use the Filebeat
  provided host.name
  instead. https://github.com/jasonish/evebox/issues/100
- Allow `esimport` to read from multiple eve files. If bookmarking is
  used, `--bookmark-dir` must be used instead of
  `--bookmark-filename`. https://github.com/jasonish/evebox/issues/98
- Support Elastic
  Search 7. https://github.com/jasonish/evebox/issues/112
- Reduce the amount of per minute logs by moving some message to debug
  (verbose) mode. https://github.com/jasonish/evebox/issues/116

### Fixes
- Show event services on first click through to event, rather than having
  to refresh to see them.
  Issue: https://github.com/jasonish/evebox/issues/109
- Fix sensor name display when event is clicked on in inbox or alert
  view. https://github.com/jasonish/evebox/issues/104

### Breaking Changes
- `esimport` now uses a default index of `logstash` instead of
  `evebox` to match common usage.
- The `evebox` application now requires a command name. It will not
  fallback to the server command anymore.
- The EveBox server will now bind to localhost by default instead of
  being open. Use the `--host` command line option to accept connections
  more openly. https://github.com/jasonish/evebox/issues/110
- GitHub authentication has been removed. Looks like its been broken for
  a little while now.

### Known Issues
- Filebeat: The basic views work with Filebeat indices but searching
  does not. This is due to Filebeat indexing fields as keywords which
  complicates "free text" searching. This will probably not be fixed,
  but instead focus will be on supporting Elastic Search ECS (or more
  simply the Suricata plugin for filebeat) -
  https://github.com/jasonish/evebox/issues/97

### Deprecations
- LetsEncrypt support: This is better done by a reverse proxy where
  LetsEncrypt support is more of a design goal.
- Plain Filebeat indices will likely be deprecated due to issues with
  searching.

[Full Changelog](https://github.com/jasonish/evebox/compare/0.10.2..0.11.0)

## 0.10.2 - 2019-01-30

### Fixed
- If EveBox is installing the Elastic Search template, re-configure
  after installation to figure out the keyword suffix instead of
  requiring EveBox to be
  restarted. https://github.com/jasonish/evebox/issues/85
- Update the Brace Javascript dependency. Fixes issue loading event
  view. https://github.com/jasonish/evebox/issues/91
- In agg reports use default min_doc_count of 1 instead of 0. Prevents
  values from showing in the report that have 0 hits, when the number
  of results in less than the number of results requested. Affects:
  Elastic Search. https://github.com/jasonish/evebox/issues/99
- Remove top rrdata from DNS report as its not really valid with DNS
  v2 alerts. Best to remove it until an alternate metric can be used
  to report on DNS responses. Closes
  https://github.com/jasonish/evebox/issues/72.
- Fixed pager button on "Events"
  view. https://github.com/jasonish/evebox/issues/92
- Fix issue with drop down event type selector on events view page
  where choosing an event type was taking users back to the index.
- Fix pcap downloads when authentication is on. This requires setting
  a cookie as this isn't an XHR/REST style request.
  https://github.com/jasonish/evebox/issues/90
- Fix doc on adding a
  user. https://github.com/jasonish/evebox/issues/89

[Full Changelog](https://github.com/jasonish/evebox/compare/0.10.1..0.10.2)

## 0.10.1 - 2018-12-20
- Fix issue when behind a path on a reverse
  proxy. https://github.com/jasonish/evebox/issues/84

[Full Changelog](https://github.com/jasonish/evebox/compare/0.10.0..0.10.1)

## 0.10.0 - 2018-12-19
- Update to Angular 7.
- Migrate to Go 1.11 module support. This requires Go 1.11, but no
  longer requires building in the GOPATH.
- Event rendering fixes.
- Allow Elastic Search index prefix and template name to be
  different. https://github.com/jasonish/evebox/issues/83

[Full Changelog](https://github.com/jasonish/evebox/compare/0.9.1...0.10.0)

## 0.9.1 - 2018-05-29
- Better Elastic Search version support, including Elastic Search 6.
- Fix rule highlight (including making reference URLs links).
- Various event view cleanups.
- [Agent] The agent will now add the rule to the alert object, the same location
  as Suricata.
- [Elastic Search] If no keyword found, use "raw" for those remaining Elastic
  Search 2 templates out there.

[Full Changelog](https://github.com/jasonish/evebox/compare/0.9.0...0.9.1)

## 0.9.0 - 2018-02-07

**Fixed**
- The inbox will not remember the sort after after archiving or
  escalating event. Indicators of sort order were added, and the sort
  order is now retained after refresh or page
  reload. https://github.com/jasonish/evebox/issues/61
- [Elastic Search] Per IP report when the src_ip and dest_ip fields
  have been mapped to the IP datatype
  (https://github.com/jasonish/evebox/issues/56)
- When parsing rules, if parse error was encountered the remaining
  rules would not be parsed. Instead log and continue parsing.
- Various fixes to oneshot where it would stop reading the input file.
- Fix eve reader getting stuck on malformed records
  (https://github.com/jasonish/evebox/issues/69)
- Various fixes to the SSH report.

**Changes**
- Upgrade the Bootstrap CSS framework to version 4.
- Include Logstash 6 template for use with Elastic Search 6.
- Convert the SSH histogram graph to bars instead of lines, in
  consideration of doing this for all histogram graphs.

**Removed**
- Support for Elastic Search versions less than 5.

[Full Changelog](https://github.com/jasonish/evebox/compare/0.8.1...0.9.0)

## 0.8.1 - 2017-12-10

**Added**
- Commenting support for PostgreSQL.
  - With "has:comment" query string support.
  - And "comment:SOME_STRING" for search comments.
- In oneshot mode, continue reading the last file to pickup new events
  (https://github.com/jasonish/evebox/issues/54).
- Add "Newer" and "Oldest" buttons to the "Events" view.

**Fixed**
- Fix an issue with updating the "active" row after archiving events.
- Strip trailing slashes in the Elastic Search URL
  (https://github.com/jasonish/evebox/issues/55).

**Changes**
- In requests to the backend, rename maxTs, minTs, eventType to
  max_ts, min_ts and event_type.

[Full Changelog](https://github.com/jasonish/evebox/compare/0.8.0...0.8.1)

## 0.8.0 - 2017-06-30

**Added**
- The agent, and the server when reading logs can now add the rule to
  the event by providing the location of the rule files in the
  configuration.
- Add option to esimport to add rule to event.
- If an event has a "rule" object it will now be displayed in the
  event details.
- Initial support for PostgreSQL. Like SQLite this does not yet
  support reporting.
- Event history recording. A timestamp and username will be recorded
  when an alert is archived, escalated or de-escalated.
- Support for commenting on events (Elastic Search only)
  (https://github.com/jasonish/evebox/issues/36).
- Specific support for displaying the HTTP response body if available
  in Eve entries. Requires Suricata 4.0.0-rc1 or newer
  (https://github.com/jasonish/evebox/issues/40)
  
**Fixed**
- Fix an issue where alerts may not be archived if their @timestamp
  and timestamp fields were out of sync -
  https://github.com/jasonish/evebox/issues/48.
- A usability issue where the alert view would be reset to 100 items
  after arching event, if previously set to "all" -
  https://github.com/jasonish/evebox/issues/49.
- Elastic Search mapping errors on flow and netflow reports -
  https://github.com/jasonish/evebox/issues/39

[Full Changelog](https://github.com/jasonish/evebox/compare/0.7.0...0.8.0)

## 0.7.0 - 2017-04-22

**Added**
- Optional authentication. Authentication can now be enabled with
  simple usernames and passwords. GitHub can also be used for
  authentication using Oauth2, however, the user must first be created
  in EveBox.
- New command, _evebox config users_, to create users.
- Create and use a "configdb". This is a database separate from event
  databases for storing data such as users. Will contain more
  configuration data in the future.
- TLS support. The server can be provided with a certificate and key
  to enable TLS. The "gencert" subcommand has been added to help
  generate self signed certificates. Or, if the server is publically
  accessible, Letsencrypt can be used.

**Breaking Changes**
- RPM and Debian package installs started with systemd now run as the
  user _evebox_. This really only matters if using an SQLite database,
  and the database file will need to have its permissions updated so
  the _evebox_ user will have read and write access to it.
- All binary builds are now linked with SQLite as SQLite is used for
  the configuration database. This really only matters when trying to
  cross compile EveBox, which may or may not work going forward.

[Full Changelog](https://github.com/jasonish/evebox/compare/0.6.1...0.7.0)

## 0.6.1 - 2017-04-02
[Full Changelog](https://github.com/jasonish/evebox/compare/0.5.0...0.6.1)
- Upgrade to Angular 4 and Angular CLI 1.0 and use its AOT compilation
  feature reducing the Javascript size even further. Combined with
  response compression, initial data loaded by the browser is about
  7-8x less.
- Compress HTTP responses speeding up initial load times.
- New "oneshot" mode - a mode where EveBox directly reads in an
  eve.log file into an SQLite database for one time viewing, then
  cleans up after itself.
- The EveBox server can now process an eve file without an agent
  (basically an embedded agent), storing the events in Elastic Search
  or SQLite
- When using Elastic Search 5.2+, use the update_by_query API to
  archive and escalate events. This should speed up archiving.
- Fix Elastic Search keyword handling when Filebeat is used to send
  eve logs directly to Elastic Search.
- Reports:
  - In addition to the event views, there are now some report views.
- EveBox Agent:
  - The EveBox agent is a replacement for Filebeat and/or Logstash. It
    can read Suricata eve log files sending them to the EveBox server
    which will then store them to the configured data store (Elastic
    Search or SQLite).
- SQLite Support:
  - SQLite can now be used as a backend. This is suitable for smaller
	installations where event load is light.
  - Reports are currently not supported with SQLite.
- If the agent is being used to submit events and the datastore is
  Elastic Search, create a template if one doesn't already index for
  the configured index. For Elastic Search 2.x and Logstash 2 template
  is used, for Elastic Search 5.x and Logstash 5 template is used.
- A start on some documentation:
  http://evebox.readthedocs.io/en/latest/index.html

## 0.5.0 - 2016-06-17
[Full Changelog](https://github.com/jasonish/evebox/compare/0.4.0...0.5.0)
- EveBox is now runs as a server instead of just some static files
  that use the browser to connect directly to Elastic Search. This
  will allow:
  - Simple setup and dealing with CORS.
  - A platform to provide new features and other database options
    moving forward.

## 0.4.0 - 2015-12-10
[Full Changelog](https://github.com/jasonish/evebox/compare/0.3.0...0.4.0)
- Lots of UI updates.

## 0.3.0 - 2014-07-30
[Full Changelog](https://github.com/jasonish/evebox/compare/0.2.0...0.3.0)
- Depends on Elastic Search 1.3.0+.
- Use Groovy for Elastic Search scripting.  Works with the default
  configuration now (no need to enable dynamic scripting).
- Use the new top hits aggregation in ES 1.3 to limit the number of
  trips to the ES to build an aggregate view.
- Display packet and payload data now available in Suricata eve logs
  (Only in Suricata git builds as of now).

## 0.2.0 - 2014-05-20
[Full Changelog](https://github.com/jasonish/evebox/compare/0.1.0...0.2.0)
- Aggregate events.
- First step at view non-alert events.

## 0.1.0 - 2014-05-22
- Initial release.

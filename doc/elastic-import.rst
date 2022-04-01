Elasticsearch Importer
======================

The EveBox "elastic-import" tool can be used to import *eve* log files
directly into Elasticsearch. For most basic use cases it can be used
as an alternative to *Filebeat* and/or *Logstash*.

EveBox "elastic-import" features:

- Continuous (tail -f style) reading of *eve* log files.
- Bookmarking of reads so reading can continue where it stopped during
  a restart.
- GeoIP lookups using the MaxMind GeoLite2 database if provided by the
  user.
- HTTP user agent parsing (currently broken: see
  https://github.com/jasonish/evebox/issues/167)
- One shot imports to send an *eve* log file to Elastic Search once.

Logstash Compatibility
----------------------

EveBox *elastic-import* is compatible with Logstash and can be used in
a mixed environment where some *eve* logs are being handled by
*Logstash* and others by *elastic-import*. In this case you will want
to use the **--index** option to set the index to the same name that
*Logstash* is importing to.

Filebeat Compatibility
----------------------

The *elastic-import* tool is not compatible with Elasticsearch indexes
created by Filebeat with or without the Filebeat Suricata module. If
using Filebeat it is not recommended to use *elastic-import* to import
Suricata events into the same indexes being used by Filebeat.

GeoIP
-----

While EveBox *elastic-import* can do geoip lookups it does not include a geoip
database. The only supported database is the MaxMind GeoLite2 database, see
http://dev.maxmind.com/geoip/geoip2/geolite2/ for more information.

.. note:: Many Linux distributions that have a geoip database package
          use the old format of the database, not the current version
          supported by MaxMind.

While the **--geoip-database** option can be used to point
*elastic-import* at the database, the following paths will be checked
automatically, in order:

* /etc/evebox/GeoLite2-City.mmdb
* /usr/local/share/GeoIP/GeoLite2-City.mmdb
* /usr/share/GeoIP/GeoLite2-City.mmdb

.. note:: MaxMind provides their own program to update the
          databases. See http://dev.maxmind.com/geoip/geoipupdate/

Updates to the geoip database on disk will be automatically picked up
by *elastic-import* every 60 seconds.

To disable geoip lookups the ``--no-geoip`` command line option can be
used.

Command Line Options
--------------------

.. option:: --config <FILENAME>

   Path to configuration file.

.. option:: --elasticsearch <URL>

   URL to the Elasticsearch server.

   Default: http://localhost:9200

.. option:: --bookmark

   Enable bookmarking of the input files. With bookmarking, the last
   read location will be remember over restarts of *elastic-import*.

.. option:: --bookmark-dir <DIRECTORY>

   Use the provided directory for bookmarks. Bookmark files will take
   the filename of the md5 of the input filename suffixed with
   `.bookmark`.

   This option is required if `--bookmark` is used with multiple
   inputs but may also be used with a single input.

.. option:: --bookmark-filename <FILENAME>

   Use the provided filename as the bookmark file. This option is only
   valid if a single input file is used.

.. option:: --index <INDEX>

   The *Elasticsearch* index prefix to add events to. The default is
   `logstash` to be compatible with *Logstash*.

   Events will be added to an index that includes the `YYYY.MM.DD` of
   the event, for example, `2021.04.13`. To use the index verbatim,
   see the ``--no-index-suffix`` command line option.

   .. note:: Previous version of `elastic-import` used a default index of
             `evebox`.

.. option:: --no-index-suffix

   Do not add the date onto the end of the index name.

.. option:: --username <USERNAME>

   Elasticsearch username if authentication is enabled.

.. option:: --password <PASSWORD>

   Elasticsearch password if authentication is enabled.

.. option:: --no-geoip

   Disable GeoIP lookups. By default GeoIP lookups are enabled of a
   GeoIP database is found.

.. option:: --geoip-database <FILENAME>

   Location of GeoIP database to use.

Configuration File
------------------

The elastic-import command can use a YAML configuration file covering most
of the command line arguments.

.. literalinclude:: ../examples/elastic-import.yaml
   :language: yaml

Example Usage
-------------

Oneshot Import of an *Eve* Log File
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The following example will send a complete *eve.json* to Elasticsearch
and exit when done::

   evebox elastic-import --elasticsearch http://elasticsearch:9200 \
      --index logstash --oneshot -v /var/log/suricata/eve.json

Continuous Import
~~~~~~~~~~~~~~~~~

This example will run *elastic-import* in continuous mode sending events to
Elastic Search as they appear in the log file. The last read location will also
be bookmarked so *elastic-import* can continue where it left off after a
restart. For many use cases this can be used instead of *Filebeat* and/or
*Logstash*.

.. code-block:: sh

   evebox elastic-import -v \
       --elasticsearch http://elasticsearch:9200 \
       --index logstash \
       --bookmark \
       --bookmark-filename /var/tmp/eve.json.bookmark \
       /var/log/suricata/eve.json

If using *elastic-import* in this way you may want to create a configuration
named **elastic-import.yaml** like:

.. code-block:: yaml

   input: /var/log/suricata/eve.json
   elasticsearch: http://elasticsearch:9200
   index: logstash
   bookmark: true
   bookmark-filename: /var/tmp/eve.json.bookmark

Then run *elastic-import* like:

.. code-block:: sh

  evebox elastic-import -c elastic-import.yaml -v
              

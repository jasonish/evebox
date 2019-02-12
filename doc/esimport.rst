Elastic Search Importer (evebox esimport)
=========================================

The EveBox "esimport" command can be used to import *eve* log files
directly into Elastic Search. For most basic use cases it can be used
as an alternative to *Filebeat* and/or *Logstash*.

EveBox "esimport" features:

- Continuous (tail -f style) reading of *eve* log files.
- Bookmarking of reads so reading can continue where it stopped during
  a restart.
- GeoIP lookups using the MaxMind GeoLite2 database if provided by the
  user.
- HTTP user agent parsing.
- One shot imports to send an *eve* log file to Elastic Search once.

Logstash Compatibility
----------------------

EveBox *esimport* is fully compatible with Logstash and can be used in
a mixed environment where some *eve* logs are being handled by
*Logstash* and others by *esimport*. In this case you will want to use
the **--index** option to set the index the same that *Logstash* is
importing to.

Elastic Search Compatible
-------------------------

EveBox *esimport* can be used with Elastic Search version 2 and 5. If
the configured *index* does not exist, *esimport* will create a
*Logstash 2* style template for *Elastic Search v2.x* and a *Logstash
5* style template for *Elastic Search v5.x* to maintain compatibility
with *eve* events imported with *Logstash*.

Example Usage
-------------

Oneshot Import of an *Eve* Log File
~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~~

The following example will send a complete eve.json to Elastic Search
and exit when done::

   evebox esimport --elasticsearch http://10.16.1.10:9200 --index logstash \
       --oneshot -v /var/log/suricata/eve.json

Continuous Import
~~~~~~~~~~~~~~~~~

This example will run *esimport* in continuous mode sending events to
Elastic Search as they appear in the log file. The last read location
will also be bookmarked so *esimport* can continue where it left off
after a restart. For many use cases this can be used instead of
*Filebeat* and/or *Logstash*.

::

   ./evebox esimport --elasticsearch http://10.16.1.10:9200 --index logstash \
       --bookmark --bookmark-filename /var/tmp/eve.json.bookmark -v \
       /var/log/suricata/eve.json

If using *esimport* in this way you may want to create a configuration
named **esimport.yaml** like:

.. code-block:: yaml

   input: /var/log/suricata/eve.json
   elasticsearch: http://10.16.1.10:9200
   index: logstash
   bookmark: true
   bookmark-filename: /var/tmp/eve.json.bookmark

Then run *esimport* like::

  ./evebox esimport -c esimport.yaml -v

GeoIP
-----

While EveBox *esimport* can do geoip lookups it does not include a
geoip database. The only supported database is the MaxMind GeoLite2
database, see http://dev.maxmind.com/geoip/geoip2/geolite2/ for more
information.

.. note:: Many Linux distributions that have a geoip database package
          use the old format of the database, not the current version
          supported by MaxMind.

While the **--geoip-database** option can be used to point *esimport*
at the datbase, the following paths will be checked automatically, in
order:

* /etc/evebox/GeoLite2-City.mmdb.gz
* /etc/evebox/GeoLite2-City.mmdb
* /usr/local/share/GeoIP/GeoLite2-City.mmdb
* /usr/share/GeoIP/GeoLite2-City.mmdb

.. note:: MaxMind provides their own program to update the
          databases. See http://dev.maxmind.com/geoip/geoipupdate/

GeoIP Quickstart
~~~~~~~~~~~~~~~~

If you just want to get quickly started with GeoIP you can download
the database to a path that *esimport* will automatically detect, for
example::

  mkdir -p /etc/evebox
  cd /etc/evebox
  curl -OL http://geolite.maxmind.com/download/geoip/database/GeoLite2-City.mmdb.gz

Command Line Options
--------------------

.. option:: --force-template

   Like Logstash's `overwrite-template`, this option will always load
   the EveBox template into Elasticsearch.

   While this option is off by default, it is recommended if only
   using EveBox to add events to Elasticsearch. It is off by default
   to better work with Elasticsearch instances where the template is
   already managed by Logstash of Filebeat.

.. option:: --bookmark

   Enable bookmarking of the input files. With bookmarking, the last
   read location will be remember over restarts of `esimport`.

.. option:: --bookmark-dir DIRECTORY

   Use the provided directory for bookmarks. Bookmark files will take
   the filename of the md5 of the input filename suffixed with
   `.bookmark`.

   This option is required if `--bookmark` is used with multiple
   inputs but may also be used with a single input.

.. option:: --bookmark-filename FILENAME

   Use the provided filename as the bookmark file. This option is only
   valid if a single input file is used.

.. option:: --index INDEX

   The *Elastic Search* index prefix to add events to. The default is
   `logstash` to be compatible with *Logstash*.

   .. note:: Previous version of `esimport` used a default index of
             `evebox`.

Configuration File
------------------

The esimport command can use a YAML configuration file covering most
of the command line arguments.

.. literalinclude:: esimport.yaml
   :language: yaml

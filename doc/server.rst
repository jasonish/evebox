Server
======

.. toctree::
   :maxdepth: 2
   :caption: Contents:

   server-running
   authentication
   tls

Command Line Options
--------------------

For a full list of the EveBox Server command line options run::

   evebox server --help

.. option:: -e, --elasticsearch <URL>

   URL to Elasticsearch server.

   Default: ``http://127.0.0.1:9200``

   Environment variable: ``EVEBOX_ELASTICSEARCH_URL``

.. option:: --host <HOSTNAME>

   Hostname or IP address to bind to.

   Default: ``127.0.0.1``

   Environment variable: ``EVEBOX_HTTP_HOST``

.. option:: -p, --port <PORT>

   Port to bind to.

   Default: ``5636``

   Environment variable: ``EVEBOX_HTTP_PORT``

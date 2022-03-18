EveBox Agent
============

The EveBox "agent" is a tool for sending *eve* events directly to an EveBox
server without the need for tools like *Filebeat* and/or *Logstash*. Events sent
with the agent are handled by the EveBox server and stored in the database by
the server.

Example Usage
-------------

Command Line Only
~~~~~~~~~~~~~~~~~

If your EveBox Server is setup without any authentication all options can be
provided on the command line. Example::

   evebox agent --server http://hostname:5636 /var/log/suricata/eve*.json

This will process all *eve* log files in ``/var/log/suricata`` and send them to
an EveBox Server hosted over at ``hostname``.

.. note::

   This will store bookmark information (the last location processed in each log
   file) in the current directory. This is OK if you will always be lanching
   EveBox Agent from the same working directory, however it is recommended to
   use the `-D` option to set the data directory to a consistent location.

With a Configuration File
~~~~~~~~~~~~~~~~~~~~~~~~~

If you need options such as a username/password for the EveBox server, want to
add additional fields to the Suricata EVE records or add the rules to the EVE
records it is recommended to use a configuration file.

A starter configuration file can be created with the following command::

   evebox print agent.yaml

You may want to redirect this to a file::

   evebox print agent.yaml > agent.yaml

Then the EveBox can be started like::

   evebox agent -c /path/to/agent.yaml

By default the EveBox Agent will first look in the current directory for
``agent.yaml`` then ``/etc/evebox/agent.yaml``.

Command Line Options
--------------------

.. program:: evebox agent

.. option:: -c, --config <FILENAME>

   Path to configuration file. If not provided the agent will look for a
   configuration named in ``agent.yaml`` in the current directory then look for
   ``/etc/evebox/agent.yaml``.

.. option:: -D, --data-directory <DIR>

   Provide a directory where the Agent can store data and other state
   information. There is no default, but providing a directory like
   ``/var/lib/evebox/agent`` and making sure that directory is writable by the
   agent is highly recommended.

.. option:: --enable-geoip

   Enables MaxMind GeoIP lookups and will add GeoIP information for events. This
   depends on the GeoIP database being up to date and available in standard
   locations.

.. _agent_server_url:

.. option:: server <URL>

   The EveBox server to connect to. Specified like ``http://1.1.1.1:5636`` or
   ``https://my-evebox-server.domain.com:5636``. Note that if using ``https``,
   the URL must use a hostname and not an IP address.

.. option:: --stdout

   Prints events to stdout. Useful for debugging.

.. option:: -v, --verbose

   Specify once for debug level logging, and 2 or more times for trace level
   logging.

.. option:: FILENAMES...

   Any filenames provided on the command line will be read by the Agent and sent
   to the EveBox server. If filenames are specified on the command the input
   files in the configuration file will be ignored.

Environment Variables
---------------------

Some configuration files can be provided with environment variables. The order
or precedence is:

* Command line arguments
* Environment variables
* Configuration file

.. envvar:: EVEBOX_SERVER_URL

   The EveBox server URL to connect to. See the documentation for the
   :ref:`--server <agent_server_url>` command line option for more information.

Configuration File
------------------

A default configuration can be generated with the command::

   evebox print agent.yaml

Default Configuration File
~~~~~~~~~~~~~~~~~~~~~~~~~~

.. literalinclude:: ../examples/agent.yaml
   :language: yaml

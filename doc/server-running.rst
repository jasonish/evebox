Running
=======

Using an Existing ELK Stack
---------------------------

Assuming you already have an existing working Suricata, Elastic
Search, Logstash and Kibana stack working, then EveBox should just
work if pointed at your Elastic Search server.

Example::

  evebox server -v -e http://elasticsearch:9200

This assumes the use of the default Logstash index
logstash-{YYYY.MM.DD}. If another index name is being used it must be
specified with the ``-i`` option::

  evebox server -v -e http://elasticsearch:9200 -i indexprefix

Consuming Events and Using Elastic Search
-----------------------------------------

If you do not have an existing ELK stack, but are able to provide
Elastic Search, EveBox can ship the events to Elastic Search itself.

Example usage::

  evebox server -v -e http://elasticsearch:9200 --input /var/log/suricata/eve.json

.. note:: If you do not wish to run EveBox on the same machine as
          Suricata you can use the :doc:`agent` to ship alerts to the
          EveBox server.

Using the Embedded SQLite Database
----------------------------------

If installing Elastic Search is not an option the embedded SQLite
database can be used instead::

  evebox server -v -D . --datastore sqlite --input /var/log/suricata/eve.json
  
.. note:: Note the -D parameter that tells EveBox where to store data
          files such as the file for the SQLite database. While using
          the current directory, or a temp directory is OK for
          testing, you may want to use something like /var/lib/evebox
          for long term use.

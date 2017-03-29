API
===

EveBox exposes an API to the web based frontend that may be useful for
other purposes. While the API is not stable yet, this is an attempt to
document endpoints that are somewhat stable.

GET /api/1/alerts
-----------------

The `alerts` endpoint returns alert groupings as seen in the EveBox
*Inbox*, *Escalated* and *Alerts* views. An individual alert group is
considered to be a grouping of a signature id, source address and
destination address with a count of the number of times that event
occurred and a time range containing the oldest occurrence of the
alert, and newest occurrence. Additionally, the most recent occurence
of the alert is returned.

In SQL terms the grouping is like ``GROUP BY signature_id, GROUP BY
src_ip, GROUP BY dest_ip``.

Query Parameters
~~~~~~~~~~~~~~~~

.. option:: time_range (or timeRange)

   Time range to limit matching alerts to. Only alerts between 'now'
   and time_range ago will be returned.

   Examples:

   - Last minute: ``60s``
   - Last hour: ``3600s``
   - Last 24 hours: ``86400s``

   At this time only the 's' unit is support for seconds.

   This paramet is not allowed with ``min_ts`` or ``max_ts``.

.. option:: min_ts

   Specify the minimum timestamp for the range of the query. Alerts
   occurence on this or after will be included.

.. option:: max_ts

   Specify the maximum timestamp for the range of the query. Alerts
   occurring before or on this time will be included.

.. option:: tags

   A list of tags that events must, or must not have. Tags are
   commented separated, and if prefixed with "-", only alerts not
   having that tag will be returned.

   The EveBox *inbox* is made of alerts that have not been archived,
   so use the value "-evebox.archived". The *escalated* view is made
   of alerts that have the "evebox.escalated" tagged and would be
   queries with a value of "evebox.escalated".

.. option:: query_string (or queryString)

   Query string alerts must match. The format of the query string
   varies depending on the datastore used.

Response Format
~~~~~~~~~~~~~~~

.. code::

   {
     "alerts": [
       {
         "count": 82,
	 "event": {
         "_id": "98ae9349-136e-11e7-bba7-d8cb8a1db3b2",
         "_index": "logstash-2017.03.28",
         "_score": null,
         "_source": {
           "@timestamp": "2017-03-28T04:25:37.808Z",
	   ...
	 },
         "maxTs": "2017-03-27T22:25:37.808514-0600",
         "minTs": "2017-03-26T23:07:22.539277-0600",
         "escalatedCount": 0
       },
       {
         ...
       }
     ]
   }

Examples
~~~~~~~~

Query the "inbox" for alerts occurring in the last 24 hours::

  curl -G http://localhost:5636/api/1/alerts \
      -d time_range=60s \
      -d tags=-archived

Query the "escalated" view::

  curl -G http://localhost:5636/api/1/alerts \
      -d tags=evebox.escalated

Query the "Alerts" view for all alerts in the last 24 hours::

  curl -G http://localhost:5636/api/1/alerts \
      -d time_range=84600s

Query alerts for all groups in the last 24 hours containing the string
"GPL ICMP_INFO"::

  curl -G http://localhost:5636/api/1/alerts \
      -d time_range=84600s -d query_string="ICMP_INFO"
      
Query for alert groups with a destination IP of 10.16.1.10 in the last
day::

  curl -G http://localhost:5636/api/1/alerts \
      -d time_range=84600s -d query_string="dest_ip:10.16.1.10"

--------------------
-- Indexes as of V3.
--------------------

CREATE INDEX IF NOT EXISTS events_timestamp_index
  ON events (timestamp);

-- Dropped in 0.17.0
-- CREATE INDEX IF NOT EXISTS events_archived_index
--   ON events (archived);

-- Dropped in 0.17.0
-- CREATE INDEX IF NOT EXISTS events_escalated_index
--   ON events (escalated);

-- Dropped in 0.17.0
-- CREATE INDEX IF NOT EXISTS events_event_type_index
--   ON events (json_extract(source, '$.event_type'));

-- Dropped in 0.17.0
-- CREATE INDEX IF NOT EXISTS events_src_ip_index
--   ON events (json_extract(source, '$.src_ip'));

-- Dropped in 0.17.0
-- CREATE INDEX IF NOT EXISTS events_dest_ip_index
--   ON events (json_extract(source, '$.dest_ip'));

CREATE INDEX IF NOT EXISTS events_alert_signature_index
  ON events (json_extract(source, '$.alert.signature'));

CREATE INDEX IF NOT EXISTS events_alert_signature_id_index
  ON events (json_extract(source, '$.alert.signature_id'));

CREATE INDEX IF NOT EXISTS events_flow_id_index
  ON events (json_extract(source, '$.flow_id'));

-- CREATE INDEX IF NOT EXISTS events_event_type_archived
--   ON events (json_extract(source, '$.event_type'),
--              archived,
--              json_extract(source, '$.alert.signature_id'),
--              json_extract(source, '$.src_ip'),
--              json_extract(source, '$.dest_ip'));
-- CREATE INDEX IF NOT EXISTS events_escalated_view_index
--   ON events (json_extract(source, '$.event_type'),
--              escalated,
--              json_extract(source, '$.alert.signature_id'),
--              json_extract(source, '$.src_ip'),
--              json_extract(source, '$.dest_ip'));

-----------------
-- 0.17.0 Updates
-----------------

-- These don't appear to be the best indexes for the inbox and
-- escalated views.
DROP INDEX IF EXISTS events_event_type_archived;
DROP INDEX IF EXISTS events_escalated_view_index;

-- This index helps with alert views as well as anything operating on
-- event type and timestamp.
CREATE INDEX IF NOT EXISTS events_event_type_timestamp_index_v1
  ON events (json_extract(source, '$.event_type'), timestamp);

-- The above index will also cover this one.
DROP INDEX IF EXISTS events_event_type_index;

-- Drop the indexes on archived and escalated. I don't think they were
-- ever being used.
DROP INDEX IF EXISTS events_archived_index;
DROP INDEX IF EXISTS events_escalated_index;

-- Index on escalated and a timestamp, this speeds up date based event
-- deletion.
CREATE INDEX IF NOT EXISTS events_escalated_v1
  ON events (escalated, timestamp);

-- Drop address based indexes in favour of address indexes that also
-- cover the timestamp in descending order. Speeds up queries using an
-- OR query for the src and dest IP addresses.
DROP INDEX IF EXISTS events_src_ip_index;
DROP INDEX IF EXISTS events_dest_ip_index;

CREATE INDEX IF NOT EXISTS events_src_ip_index_v1
  ON events (json_extract(source, '$.src_ip'), timestamp DESC);

CREATE INDEX IF NOT EXISTS events_dest_ip_index_v1
  ON events (json_extract(source, '$.dest_ip'), timestamp DESC);

-- Speeds up inbox and archive from inbox queries/executions.
create index if not exists events_archive_index_v1
  on events (
    archived,
    json_extract(source, '$.alert.signature_id'),
    json_extract(source, '$.src_ip'),
    json_extract(source, '$.dest_ip'),
    timestamp)
  where json_extract(source, '$.event_type') = 'alert'
    and archived = 0;

-- Speeds up querying by sensor name.
create index if not exists events_sensors_timestamp_index_v1 on events (
  json_extract(source, '$.host'), json_extract(source, '$.event_type'), timestamp);

--
-- V3 Drops
--

-- DROP INDEX IF EXISTS events_timestamp_index;
-- DROP INDEX IF EXISTS events_archived_index;
-- DROP INDEX IF EXISTS events_escalated_index;
-- DROP INDEX IF EXISTS events_event_type_index;
-- DROP INDEX IF EXISTS events_src_ip_index;
-- DROP INDEX IF EXISTS events_dest_ip_index;
-- DROP INDEX IF EXISTS events_alert_signature_index;
-- DROP INDEX IF EXISTS events_alert_signature_id_index;
-- DROP INDEX IF EXISTS events_flow_id_index;
-- DROP INDEX IF EXISTS events_event_type_archived;
-- DROP INDEX IF EXISTS events_escalated_view_index;

--
-- 0.17.0 Drops
--

-- DROP INDEX IF EXISTS events_event_type_timestamp_index_v1
-- DROP INDEX IF EXISTS events_escalated_v1;
-- DROP INDEX IF EXISTS events_src_ip_index_v1;
-- DROP INDEX IF EXISTS events_dest_ip_index_v1;

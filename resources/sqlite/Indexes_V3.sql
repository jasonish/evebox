-- Indexes as of V3.

CREATE INDEX IF NOT EXISTS events_timestamp_index
  ON events (timestamp);

CREATE INDEX IF NOT EXISTS events_archived_index
  ON events (archived);

CREATE INDEX IF NOT EXISTS events_escalated_index
  ON events (escalated);

CREATE INDEX IF NOT EXISTS events_event_type_index
  ON events (json_extract(source, '$.event_type'));

CREATE INDEX IF NOT EXISTS events_src_ip_index
  ON events (json_extract(source, '$.src_ip'));

CREATE INDEX IF NOT EXISTS events_dest_ip_index
  ON events (json_extract(source, '$.dest_ip'));

CREATE INDEX IF NOT EXISTS events_alert_signature_index
  ON events (json_extract(source, '$.alert.signature'));

CREATE INDEX IF NOT EXISTS events_alert_signature_id_index
  ON events (json_extract(source, '$.alert.signature_id'));

CREATE INDEX IF NOT EXISTS events_flow_id_index
  ON events (json_extract(source, '$.flow_id'));

CREATE INDEX IF NOT EXISTS events_event_type_archived
  ON events (json_extract(source, '$.event_type'),
             archived,
             json_extract(source, '$.alert.signature_id'),
             json_extract(source, '$.src_ip'),
             json_extract(source, '$.dest_ip'));
CREATE INDEX IF NOT EXISTS events_escalated_view_index
  ON events (json_extract(source, '$.event_type'),
             escalated,
             json_extract(source, '$.alert.signature_id'),
             json_extract(source, '$.src_ip'),
             json_extract(source, '$.dest_ip'));

--

DROP INDEX IF EXISTS events_timestamp_index;
DROP INDEX IF EXISTS events_archived_index;
DROP INDEX IF EXISTS events_escalated_index;
DROP INDEX IF EXISTS events_event_type_index;
DROP INDEX IF EXISTS events_src_ip_index;
DROP INDEX IF EXISTS events_dest_ip_index;
DROP INDEX IF EXISTS events_alert_signature_index;
DROP INDEX IF EXISTS events_alert_signature_id_index;
DROP INDEX IF EXISTS events_flow_id_index;
DROP INDEX IF EXISTS events_event_type_archived;
DROP INDEX IF EXISTS events_escalated_view_index;

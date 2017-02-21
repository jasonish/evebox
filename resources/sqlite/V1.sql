CREATE TABLE events (
  id        TEXT PRIMARY KEY,
  timestamp DATETIME,
  archived  INTEGER,
  escalated INTEGER,
  source    JSON
);

CREATE INDEX events_timestamp ON events (timestamp);

CREATE INDEX events_archived ON events (archived);

CREATE INDEX events_event_type
  ON events (json_extract(source, '$.event_type'));

CREATE INDEX events_src_ip
  ON events (json_extract(source, '$.src_ip'));

CREATE INDEX events_dest_ip
  ON events (json_extract(source, '$.dest_ip'));

CREATE INDEX events_alert_signature_index
  ON events (json_extract(source, '$.alert.signature'));

CREATE INDEX events_alert_signature_id_index
  ON events (json_extract(source, '$.alert.signature_id'));

CREATE VIRTUAL TABLE events_fts USING fts5(id, timestamp, source);

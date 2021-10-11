CREATE TABLE events (
  -- Implicit rowid column is the event id and primary key.

  -- Timestamp in nanoseconds since the epoch.
  timestamp INTEGER NOT NULL,

  -- The archived flag is stored as a column as I don't think you can update
  -- json fields with an update statement.
  archived  INTEGER DEFAULT 0,

  -- Escalated/starred is also a column for the same reason as archived.
  escalated INTEGER DEFAULT 0,

  -- The actual event.
  source    JSON
);

CREATE INDEX events_timestamp_index
  ON events (timestamp);

CREATE INDEX events_archived_index
  ON events (archived);

CREATE INDEX events_escalated_index
  ON events (escalated);

CREATE INDEX events_event_type_index
  ON events (json_extract(source, '$.event_type'));

CREATE INDEX events_src_ip_index
  ON events (json_extract(source, '$.src_ip'));

CREATE INDEX events_dest_ip_index
  ON events (json_extract(source, '$.dest_ip'));

CREATE INDEX events_alert_signature_index
  ON events (json_extract(source, '$.alert.signature'));

CREATE INDEX events_alert_signature_id_index
  ON events (json_extract(source, '$.alert.signature_id'));

CREATE INDEX events_flow_id_index
  ON events (json_extract(source, '$.flow_id'));

-- Create a content-less full text search table...
CREATE VIRTUAL TABLE events_fts USING fts5(source, content = '');

-- Deleting from a content-less table requires a trigger like this.
CREATE TRIGGER events_delete
AFTER DELETE ON events
BEGIN
  INSERT INTO events_fts (events_fts, rowid) VALUES ('delete', old.rowid);
END;

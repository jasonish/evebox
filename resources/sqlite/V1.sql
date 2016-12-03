CREATE TABLE events (
  id        TEXT PRIMARY KEY,
  timestamp DATETIME,
  archived  INTEGER,
  escalated INTEGER,
  source    JSON
);

-- Index alert.signature
-- - Inbox and alert views group by alert.signature.
--
CREATE INDEX events_alert_signature_index
  ON events (json_extract(source, '$.alert.signature'));

-- Index alert.signature_id
-- - Speeds up archiving where signature_id field in the where.
CREATE INDEX events_alert_signature_id_index
  ON events (json_extract(source, '$.alert.signature_id'));

CREATE VIRTUAL TABLE events_fts USING fts5(id, source);

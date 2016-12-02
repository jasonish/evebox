CREATE TABLE events (
  id        TEXT PRIMARY KEY,
  timestamp DATETIME,
  archived  INTEGER,
  escalated INTEGER,
  source    JSON
);

-- Index on alert signature.
CREATE INDEX events_alert_signature_index
  ON events (json_extract(source, '$.alert.signature'));

CREATE VIRTUAL TABLE events_fts USING fts5(id, source);

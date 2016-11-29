CREATE TABLE events (
  id        TEXT PRIMARY KEY,
  timestamp TEXT,
  archived  INTEGER,
  escalated INTEGER,
  source    JSON
);

CREATE INDEX events_timestamp_index
  ON events (timestamp);

CREATE VIRTUAL TABLE events_fts USING fts5(id, source);

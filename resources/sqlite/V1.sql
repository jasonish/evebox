CREATE TABLE events (
  id        TEXT PRIMARY KEY,
  timestamp DATETIME,
  archived  INTEGER DEFAULT 0,
  escalated INTEGER DEFAULT 0,
  source    JSON
);

CREATE INDEX events_timestamp
  ON events (timestamp);

CREATE INDEX events_archived
  ON events (archived);

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

-- Example inbox search...
-- SELECT
--   b.count,
--   a.id,
--   b.escalated_count,
--   a.archived,
--   a.timestamp,
--   a.source
-- FROM events a
--   INNER JOIN
--   (
--     SELECT
--       events.rowid,
--       count(json_extract(events.source, '$.alert.signature_id')) AS count,
--       max(timestamp)                                             AS maxts,
--       sum(
--           escalated)                                             AS escalated_count
--     FROM events, events_fts
--     WHERE json_extract(events.source, '$.event_type') = 'alert'
--           AND archived = 0
--           AND events_fts MATCH '"zero"'
--           AND events.rowid = events_fts.rowid
--     GROUP BY
--       json_extract(events.source, '$.alert.signature_id'),
--       json_extract(events.source, '$.src_ip'),
--       json_extract(events.source, '$.dest_ip')
--   ) AS b
-- WHERE a.rowid = b.rowid
--       AND a.timestamp = b.maxts
-- ORDER BY timestamp
--   DESC

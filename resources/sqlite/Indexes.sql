-- Drop indexes no longer used.
DROP INDEX IF EXISTS events_event_type_archived;
DROP INDEX IF EXISTS events_escalated_view_index;
DROP INDEX IF EXISTS events_event_type_index;
DROP INDEX IF EXISTS events_archived_index;
DROP INDEX IF EXISTS events_escalated_index;
DROP INDEX IF EXISTS events_src_ip_index;
DROP INDEX IF EXISTS events_dest_ip_index;

CREATE INDEX IF NOT EXISTS events_timestamp_index
  ON events (
    timestamp
);

CREATE INDEX IF NOT EXISTS events_alert_signature_index
  ON events (
    json_extract(source, '$.alert.signature')
);

CREATE INDEX IF NOT EXISTS events_alert_signature_id_index
  ON events (
    json_extract(source, '$.alert.signature_id')
);

CREATE INDEX IF NOT EXISTS events_flow_id_index
  ON events (
    json_extract(source, '$.flow_id')
);

-- Slow when fetching non-archived events one by one in descending
-- timestamp order. And can't detect when we've hit the last
-- non-archived event.
--
-- Replaced by: index_events_event_type_v2
-- Removed in: 0.19
-- CREATE INDEX events_event_type_timestamp_index_v1
--   ON events (
--     json_extract(source, '$.event_type'),
--     timestamp
-- );

-- Should be able to drop this one.
CREATE INDEX IF NOT EXISTS events_escalated_v1
  ON events (
    escalated,
    timestamp
);

CREATE INDEX IF NOT EXISTS events_src_ip_index_v1
  ON events (
    json_extract(source, '$.src_ip'),
    timestamp DESC
);

CREATE INDEX IF NOT EXISTS events_dest_ip_index_v1
  ON events (
    json_extract(source, '$.dest_ip'),
    timestamp DESC
);

CREATE INDEX IF NOT EXISTS events_archive_index_v1
  ON events (
    archived,
    json_extract(source, '$.alert.signature_id'),
    json_extract(source, '$.src_ip'),
    json_extract(source, '$.dest_ip'),
    timestamp)
  WHERE json_extract(source, '$.event_type') = 'alert'
    AND archived = 0;

-- Fast query of distinct host names.
-- Fast query of events by sensor.
CREATE INDEX events_sensors_timestamp_index_v1 on events (
  json_extract(source, '$.host'),
  json_extract(source, '$.event_type'),
  timestamp
);

---
--- 0.19
---

-- Replaces: events_event_type_timestamp_index_v1
DROP INDEX IF EXISTS events_event_type_timestamp_index_v1;
CREATE INDEX index_events_event_type_v2
  ON events (
    json_extract(source, '$.event_type'), 
    timestamp,
    archived,
    escalated
);

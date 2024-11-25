-- Existed in 0.18.
CREATE INDEX IF NOT EXISTS events_timestamp_index
  ON events (
    timestamp
);

-- Existed in 0.18.
CREATE INDEX IF NOT EXISTS events_alert_signature_index
  ON events (
    json_extract(source, '$.alert.signature')
);

-- Existed in 0.18.
CREATE INDEX IF NOT EXISTS events_alert_signature_id_index
  ON events (
    json_extract(source, '$.alert.signature_id')
);

-- Existed in 0.18.
CREATE INDEX IF NOT EXISTS events_flow_id_index
  ON events (
    json_extract(source, '$.flow_id')
);

-- Existed in 0.18.
CREATE INDEX IF NOT EXISTS events_escalated_v1
  ON events (
    escalated,
    timestamp
);

-- Existed in 0.18.
CREATE INDEX IF NOT EXISTS events_src_ip_index_v1
  ON events (
    json_extract(source, '$.src_ip'),
    timestamp DESC
);

-- Existed in 0.18.
CREATE INDEX IF NOT EXISTS events_dest_ip_index_v1
  ON events (
    json_extract(source, '$.dest_ip'),
    timestamp DESC
);

-- Existed in 0.18.
CREATE INDEX IF NOT EXISTS events_archive_index_v1
  ON events (
    archived,
    json_extract(source, '$.alert.signature_id'),
    json_extract(source, '$.src_ip'),
    json_extract(source, '$.dest_ip'),
    timestamp)
  WHERE json_extract(source, '$.event_type') = 'alert'
    AND archived = 0;

-- Existed in 0.18.
CREATE INDEX IF NOT EXISTS events_sensors_timestamp_index_v1
  ON events (
    json_extract(source, '$.host'),
    json_extract(source, '$.event_type'),
    timestamp
);

-- New in 0.19.
-- Replaces: events_event_type_timestamp_index_v1
CREATE INDEX IF NOT EXISTS index_events_event_type_v2
  ON events (
    json_extract(source, '$.event_type'), 
    timestamp,
    archived,
    escalated
);

--
-- Changelog
--
-- 0.19.0
-- - Drop: events_event_type_timestamp_index_v1
-- - Add: index_events_event_type_v2

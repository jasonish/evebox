-- Really speeds up the event views, and helps with archiving from the inbox.
DROP INDEX IF EXISTS events_event_type_archived;
CREATE INDEX events_event_type_archived
  ON events (json_extract(source, '$.event_type'),
             archived,
             json_extract(source, '$.alert.signature_id'),
             json_extract(source, '$.src_ip'),
             json_extract(source, '$.dest_ip'));


DROP INDEX IF EXISTS events_escalated_view_index;
CREATE INDEX events_escalated_view_index
  ON events (json_extract(source, '$.event_type'),
             escalated,
             json_extract(source, '$.alert.signature_id'),
             json_extract(source, '$.src_ip'),
             json_extract(source, '$.dest_ip'));

ANALYZE;

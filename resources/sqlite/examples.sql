-- DHCP Report
SELECT json_extract(events.source, '$.timestamp'),
       json_extract(events.source, '$.dhcp.client_mac'),
       json_extract(events.source, '$.dhcp.assigned_ip'),
       json_extract(events.source, '$.dhcp.hostname')
FROM events
WHERE json_extract(events.source, '$.event_type') = 'dhcp'
  AND json_extract(events.source, '$.dhcp.type') = 'reply'
ORDER BY timestamp DESC
;

-- DHCP Servers
SELECT DISTINCT json_extract(events.source, '$.src_ip')
FROM events
WHERE json_extract(events.source, '$.event_type') = 'dhcp'
  AND json_extract(events.source, '$.dhcp.type') = 'reply'
;
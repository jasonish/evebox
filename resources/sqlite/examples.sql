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

-- Stats: Date from timestamp, selecting the max value in each one minute interval.
SELECT strftime('%Y%m%d%H%M', timestamp / 1000000000, 'unixepoch') AS d
     , MAX(json_extract(events.source, '$.stats.capture.kernel_packets'))
FROM events
WHERE json_extract(events.source, '$.event_type') = 'stats'
GROUP BY d
;

--- Group into 5 minute buckets.
SELECT datetime((timestamp / 1000000000 / 300) * 300, 'unixepoch') AS d
     , MAX(json_extract(events.source, '$.stats.capture.kernel_packets'))
FROM events
WHERE json_extract(events.source, '$.event_type') = 'stats'
GROUP BY d
;

--- Group into 5 minute buckets.
SELECT datetime((timestamp / 1000000000 / 300) * 300, 'unixepoch') AS d
     , SUM(json_extract(events.source, '$.stats.flow.memuse'))
FROM events
WHERE json_extract(events.source, '$.event_type') = 'stats'
GROUP BY d
;

-- 
select * from events
where json_extract(source, '$.event_type') = 'alert' 
  and archived = 0 
order by timestamp desc;

select * from events
where json_extract(source, '$.event_type') = 'alert' 
  and escalated = 1
order by timestamp desc;

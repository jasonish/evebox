-- Archive events.
UPDATE events
SET tags = array_append(tags, 'archived')
WHERE (NOT tags && ARRAY ['archived,asdf']);

-- Escalate events.
--update events set tags = array_prepend('escalated', tags) where (not tags @> array['escalated']);
UPDATE events
SET tags = array_append(tags, 'escalated')
WHERE (NOT tags && ARRAY ['escalated']);

-- Unstar events.
--update events set tags = array_remove(tags, 'escalated') where tags @> array['escalated'];
UPDATE events
SET tags = array_remove(tags, 'escalated')
WHERE (tags && ARRAY ['escalated']);

SELECT DISTINCT ON (maxts, sigid, grouped.src_ip, grouped.dest_ip)
  grouped.count,
  grouped.escalated_count,
  events_source.uuid,
  grouped.maxts,
  grouped.mints,
  events_source.source,
  events.tags
FROM (SELECT
        count(events_source.source -> 'alert' ->> 'signature_id')      AS count,
        count(CASE WHEN tags && ARRAY ['escalated']
          THEN 1 END)                                                  AS escalated_count,
        max(events_source.timestamp)                                   AS maxts,
        min(events_source.timestamp)                                   AS mints,
        (events_source.source -> 'alert' ->> 'signature_id') :: BIGINT AS sigid,
        (events_source.source ->>
         'src_ip') :: INET                                             AS src_ip,
        (events_source.source ->>
         'dest_ip') :: INET                                            AS dest_ip
      FROM events_source, events
      WHERE events_source.source ->> 'event_type' = 'alert' AND
            events.uuid = events_source.uuid
      GROUP BY (events_source.source -> 'alert' ->> 'signature_id') :: BIGINT,
        (events_source.source ->> 'src_ip') :: INET,
        (events_source.source ->> 'dest_ip') :: INET) AS grouped
  JOIN events_source ON events_source.timestamp = grouped.maxts AND
                        (events_source.source -> 'alert' ->>
                         'signature_id') :: BIGINT = grouped.sigid AND
                        (events_source.source ->> 'src_ip') :: INET =
                        grouped.src_ip AND
                        (events_source.source ->> 'dest_ip') :: INET =
                        grouped.dest_ip
  , events
WHERE events.uuid = events_source.uuid
ORDER BY maxts DESC;

EXPLAIN
SELECT
  grouped.count,
  grouped.escalated_count,
  events.uuid,
  grouped.maxts AS maxts,
  grouped.mints,
  events_source.source,
  events.tags
FROM (
       SELECT
         count(s.source -> 'alert' ->> 'signature_id') :: BIGINT AS count,
         count(CASE WHEN events.tags && ARRAY ['escalated']
           THEN 1 END)                                           AS escalated_count,
         max(events.timestamp)                                   AS maxts,
         min(events.timestamp)                                   AS mints,
         (s.source -> 'alert' ->> 'signature_id') :: BIGINT      AS sig_id,
         (s.source ->> 'src_ip') :: INET                         AS src_ip,
         (s.source ->> 'dest_ip') :: INET                        AS dest_ip
       FROM events, events_source AS s
       WHERE events.uuid = s.uuid
             AND s.source ->> 'event_type' = 'alert'
             AND (archived = FALSE OR archived IS NULL)
       -- AND (NOT events.tags && ARRAY ['archived'])
       GROUP BY (S.source -> 'alert' ->> 'signature_id') :: BIGINT,
         (S.source ->> 'src_ip') :: INET,
         (S.source ->> 'dest_ip') :: INET
     ) AS grouped
  JOIN events_source
    ON events_source.timestamp = grouped.maxts
       AND events_source.source ->> 'event_type' = 'alert'
       AND (events_source.source -> 'alert' ->> 'signature_id') :: BIGINT =
           grouped.sig_id
       AND (events_source.source ->> 'src_ip') :: INET = grouped.src_ip
       AND (events_source.source ->> 'dest_ip') :: INET = grouped.dest_ip
  , events
WHERE events.uuid = events_source.uuid
      AND (archived = FALSE OR archived IS NULL)
ORDER BY maxts DESC;

UPDATE events
SET tags = array_append(tags, 'archived')
FROM events_source
WHERE
  (NOT tags && ARRAY ['archived'])
  AND events_source.source ->> 'event_type' = 'alert'
  AND (events_source.source ->> 'src_ip') :: INET = '10.16.1.1'
  AND (events_source.source ->> 'dest_ip') :: INET = '10.16.1.2'
  AND (events_source.source -> 'alert' ->> 'signature_id') :: BIGINT = 10000
  AND events_source.timestamp <= now()
  AND events_source.timestamp >= now() - INTERVAL '1 day'
  AND events.uuid = events_source.uuid;


EXPLAIN SELECT
          grouped.count,
          grouped.escalated_count,
          events.uuid,
          grouped.maxts AS maxts,
          grouped.mints,
          events_source.source,
          events.tags
        FROM (SELECT
                count(events_source.source -> 'alert' ->>
                      'signature_id')      AS count,
                count(CASE WHEN events.tags && ARRAY ['escalated']
                  THEN 1 END)              AS escalated_count,
                max(events.timestamp)      AS maxts,
                min(events.timestamp)      AS mints,
                (events_source.source -> 'alert' ->>
                 'signature_id') :: BIGINT AS sig_id,
                (events_source.source ->>
                 'src_ip') :: INET         AS src_ip,
                (events_source.source ->>
                 'dest_ip') :: INET        AS dest_ip
              FROM events, events_source
              WHERE events.uuid = events_source.uuid AND
                    events_source.source ->> 'event_type' = 'alert' AND
                    (NOT events.tags && ARRAY ['archived']) AND
                    events_source.timestamp > '2017-06-04T14:44:34.919809-0600'
                    AND
                    events.timestamp > '2017-06-04T14:44:34.919809-0600'
              GROUP BY
                (events_source.source -> 'alert' ->> 'signature_id') :: BIGINT,
                (events_source.source ->> 'src_ip') :: INET,
                (events_source.source ->> 'dest_ip') :: INET) AS grouped
          JOIN events_source ON events_source.timestamp = grouped.maxts AND
                                events_source.source ->> 'event_type' = 'alert'
                                AND
                                (events_source.source -> 'alert' ->>
                                 'signature_id') :: BIGINT = grouped.sig_id AND
                                (events_source.source ->> 'src_ip') :: INET =
                                grouped.src_ip AND
                                (events_source.source ->> 'dest_ip') :: INET =
                                grouped.dest_ip
                                AND events_source.timestamp >
                                    '2017-06-04T14:44:34.919809-0600'
          , events
        WHERE
          events.uuid = events_source.uuid AND
          (NOT events.tags && ARRAY ['archived'])
          AND events.timestamp > '2017-06-04T14:44:34.919809-0600'
        ORDER BY maxts DESC;

SELECT DISTINCT ON (maxts, grouped.sig_id, grouped.src_ip, grouped.dest_ip)
  grouped.count,
  grouped.escalated_count,
  events.uuid,
  grouped.maxts AS maxts,
  grouped.mints,
  events_source.source
FROM (SELECT
        count(events_source.source -> 'alert' ->> 'signature_id') AS count,
        count(CASE WHEN events.escalated = TRUE
          THEN 1 END)                                             AS escalated_count,
        max(events.timestamp)                                     AS maxts,
        min(events.timestamp)                                     AS mints,
        (events_source.source -> 'alert' ->>
         'signature_id') :: BIGINT                                AS sig_id,
        (events_source.source ->>
         'src_ip') :: INET                                        AS src_ip,
        (events_source.source ->>
         'dest_ip') :: INET                                       AS dest_ip
      FROM events, events_source
      WHERE events.uuid = events_source.uuid AND
            events_source.source ->> 'event_type' = 'alert' AND
            events.escalated = TRUE
      GROUP BY (events_source.source -> 'alert' ->> 'signature_id') :: BIGINT,
        (events_source.source ->> 'src_ip') :: INET,
        (events_source.source ->> 'dest_ip') :: INET) AS grouped
  JOIN events_source ON events_source.timestamp = grouped.maxts AND
                        events_source.source ->> 'event_type' = 'alert' AND
                        (events_source.source -> 'alert' ->>
                         'signature_id') :: BIGINT = grouped.sig_id AND
                        (events_source.source ->> 'src_ip') :: INET =
                        grouped.src_ip AND
                        (events_source.source ->> 'dest_ip') :: INET =
                        grouped.dest_ip
  , events
WHERE events.uuid = events_source.uuid AND events.escalated = TRUE
ORDER BY maxts DESC;

SELECT DISTINCT ON (maxts, grouped.sig_id, grouped.src_ip, grouped.dest_ip)
  grouped.count           AS count,
  events.archived         AS archived,
  grouped.archived_count  AS archived_count_count,
  grouped.escalated_count AS escalated_count,
  events.uuid             AS uuid,
  grouped.maxts           AS maxts,
  grouped.mints           AS mints,
  events_source.source
FROM (SELECT
        count(events_source.source -> 'alert' ->> 'signature_id') AS count,
        count(CASE WHEN events.escalated = TRUE
          THEN 1 END)                                             AS escalated_count,
        count(CASE WHEN events.archived = TRUE
          THEN 1 END)                                             AS archived_count,
        max(events.timestamp)                                     AS maxts,
        min(events.timestamp)                                     AS mints,
        (events_source.source -> 'alert' ->>
         'signature_id') :: BIGINT                                AS sig_id,
        (events_source.source ->>
         'src_ip') :: INET                                        AS src_ip,
        (events_source.source ->>
         'dest_ip') :: INET                                       AS dest_ip
      FROM events, events_source
      WHERE events.uuid = events_source.uuid AND
            events_source.source ->> 'event_type' = 'alert'
      GROUP BY (events_source.source -> 'alert' ->> 'signature_id') :: BIGINT,
        (events_source.source ->> 'src_ip') :: INET,
        (events_source.source ->> 'dest_ip') :: INET) AS grouped
  JOIN events_source ON events_source.timestamp = grouped.maxts AND
                        events_source.source ->> 'event_type' = 'alert' AND
                        (events_source.source -> 'alert' ->>
                         'signature_id') :: BIGINT = grouped.sig_id AND
                        (events_source.source ->> 'src_ip') :: INET =
                        grouped.src_ip AND
                        (events_source.source ->> 'dest_ip') :: INET =
                        grouped.dest_ip
  JOIN events
    ON events.uuid = events_source.uuid AND events.timestamp = grouped.maxts
ORDER BY maxts DESC;

DROP VIEW xevents_source;
CREATE OR REPLACE VIEW xevents_source AS
  SELECT
    uuid,
    timestamp,
    (source -> 'alert' ->> 'signature_id') :: BIGINT AS signature_id,
    (source ->> 'src_ip') :: INET                    AS src_ip,
    (source ->> 'dest_ip') :: INET                   AS dest_ip
  FROM events_source
  WHERE source ->> 'event_type' = 'alert';

SELECT *
FROM (SELECT
        count(signature_id),
        signature_id,
        max(timestamp) AS maxts,
        min(timestamp) AS mints
      FROM xevents_source
      GROUP BY signature_id, src_ip, dest_ip) AS a
  JOIN xevents_source b
    ON a.maxts = b.timestamp AND a.signature_id = b.signature_id
  JOIN events c ON b.uuid = c.uuid;

SELECT DISTINCT ON (maxts, grouped.sig_id, grouped.src_ip, grouped.dest_ip)
  grouped.count           AS count,
  grouped.escalated_count AS escalated_count,
  events.uuid             AS uuid,
  grouped.maxts           AS maxts,
  grouped.mints           AS mints,
  events_source.source,
  grouped.archived_count  AS archived_count,
  events.archived         AS archived,
  metadata ->> 'history'  AS history
FROM (SELECT
        count(events_source.source -> 'alert' ->> 'signature_id')      AS count,
        count(CASE WHEN events.escalated = TRUE
          THEN 1 END)                                                  AS escalated_count,
        count(CASE WHEN events.archived = TRUE
          THEN 1 END)                                                  AS archived_count,
        max(events.timestamp)                                          AS maxts,
        min(events.timestamp)                                          AS mints,
        (events_source.source -> 'alert' ->>
         'signature_id') :: BIGINT                                     AS sig_id,
        (events_source.source ->>
         'src_ip') :: INET                                             AS src_ip,
        (events_source.source ->>
         'dest_ip') :: INET                                            AS dest_ip
      FROM events, events_source
      WHERE events.uuid = events_source.uuid AND
            events_source.source ->> 'event_type' = 'alert' AND
            events.archived = FALSE AND events_source.timestamp >=
                                        '2017-06-12T07:50:59.779264-0600' :: TIMESTAMPTZ
            AND
            events.timestamp >= '2017-06-12T07:50:59.779264-0600' :: TIMESTAMPTZ
      GROUP BY (events_source.source -> 'alert' ->> 'signature_id') :: BIGINT,
        (events_source.source ->> 'src_ip') :: INET,
        (events_source.source ->> 'dest_ip') :: INET) AS grouped
  JOIN events_source ON events_source.timestamp = grouped.maxts AND
                        events_source.source ->> 'event_type' = 'alert' AND
                        (events_source.source -> 'alert' ->>
                         'signature_id') :: BIGINT = grouped.sig_id AND
                        (events_source.source ->> 'src_ip') :: INET =
                        grouped.src_ip AND
                        (events_source.source ->> 'dest_ip') :: INET =
                        grouped.dest_ip
  AND events_source.timestamp >= '2017-06-12T07:50:59.779264-0600' :: TIMESTAMPTZ
  , events
WHERE events.uuid = events_source.uuid AND events.archived = FALSE AND
      events.timestamp >= '2017-06-12T07:50:59.779264-0600' :: TIMESTAMPTZ
ORDER BY maxts DESC;

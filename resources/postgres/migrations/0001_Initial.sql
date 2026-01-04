-- SPDX-FileCopyrightText: (C) 2020-2025 Jason Ish <jason@codemonkey.net>
-- SPDX-License-Identifier: MIT

-- Events table - partitioned by day for efficient retention management.
-- Partitions are created based on the timestamp field (nanoseconds since epoch).
CREATE TABLE events (
    -- Timestamp in nanoseconds since the epoch (partition key)
    timestamp BIGINT NOT NULL,

    -- Primary key within partition
    rowid BIGSERIAL,

    -- The archived flag
    archived INTEGER DEFAULT 0,

    -- Escalated/starred flag
    escalated INTEGER DEFAULT 0,

    -- The actual event as JSONB for efficient querying
    source JSONB,

    -- Textual column containing values extracted from JSON for full text search
    source_values TEXT,

    -- History/comments as JSON array
    history JSONB DEFAULT '[]'::jsonb,

    -- Primary key must include partition key
    PRIMARY KEY (timestamp, rowid)
) PARTITION BY RANGE (timestamp);

-- Basic column indexes (will be created on each partition)
CREATE INDEX events_archived_idx ON events (archived);
CREATE INDEX events_escalated_idx ON events (escalated);

-- Indexes on JSON fields
CREATE INDEX events_event_type_idx ON events ((source->>'event_type'));
CREATE INDEX events_src_ip_idx ON events ((source->>'src_ip'));
CREATE INDEX events_dest_ip_idx ON events ((source->>'dest_ip'));
CREATE INDEX events_alert_signature_idx ON events ((source->'alert'->>'signature'));
CREATE INDEX events_alert_signature_id_idx ON events ((source->'alert'->>'signature_id'));
CREATE INDEX events_flow_id_idx ON events ((source->>'flow_id'));

-- Composite indexes for event views and archiving
CREATE INDEX events_event_type_archived_idx ON events (
    (source->>'event_type'),
    archived,
    (source->'alert'->>'signature_id'),
    (source->>'src_ip'),
    (source->>'dest_ip')
);

CREATE INDEX events_escalated_view_idx ON events (
    (source->>'event_type'),
    escalated,
    (source->'alert'->>'signature_id'),
    (source->>'src_ip'),
    (source->>'dest_ip')
);

-- Full text search index on source_values
CREATE INDEX events_source_values_fts_idx ON events 
    USING gin (to_tsvector('simple', coalesce(source_values, '')));

-- Index on host field to improve performance of sensor list query
CREATE INDEX events_timestamp_host_idx ON events (timestamp, (source->>'host'));

-- Constants for nanosecond calculations
-- 1 day = 86400 seconds = 86,400,000,000,000 nanoseconds

-- Function to calculate the start of a day (in nanoseconds) from a nanosecond timestamp
CREATE OR REPLACE FUNCTION evebox_day_start_nanos(ts_nanos BIGINT)
RETURNS BIGINT AS $$
DECLARE
    nanos_per_day BIGINT := 86400000000000;
BEGIN
    RETURN (ts_nanos / nanos_per_day) * nanos_per_day;
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Function to get partition name for a given nanosecond timestamp
CREATE OR REPLACE FUNCTION evebox_partition_name(ts_nanos BIGINT)
RETURNS TEXT AS $$
DECLARE
    ts_seconds BIGINT;
    partition_date DATE;
BEGIN
    ts_seconds := ts_nanos / 1000000000;
    partition_date := TO_TIMESTAMP(ts_seconds)::DATE;
    RETURN 'events_' || TO_CHAR(partition_date, 'YYYY_MM_DD');
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Function to create a partition for a given nanosecond timestamp if it doesn't exist
-- Returns the partition name
CREATE OR REPLACE FUNCTION evebox_ensure_partition(ts_nanos BIGINT)
RETURNS TEXT AS $$
DECLARE
    partition_name TEXT;
    day_start BIGINT;
    day_end BIGINT;
    nanos_per_day BIGINT := 86400000000000;
BEGIN
    partition_name := evebox_partition_name(ts_nanos);
    day_start := evebox_day_start_nanos(ts_nanos);
    day_end := day_start + nanos_per_day;

    -- Check if partition exists
    IF NOT EXISTS (
        SELECT 1 FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE c.relname = partition_name
        AND n.nspname = current_schema()
    ) THEN
        -- Create the partition
        EXECUTE format(
            'CREATE TABLE %I PARTITION OF events FOR VALUES FROM (%s) TO (%s)',
            partition_name,
            day_start,
            day_end
        );
        RAISE NOTICE 'Created partition: %', partition_name;
    END IF;

    RETURN partition_name;
END;
$$ LANGUAGE plpgsql;

-- Function to drop partitions older than a given number of days
-- Returns the number of partitions dropped
CREATE OR REPLACE FUNCTION evebox_drop_old_partitions(retention_days INTEGER)
RETURNS INTEGER AS $$
DECLARE
    cutoff_nanos BIGINT;
    partition_record RECORD;
    dropped_count INTEGER := 0;
    nanos_per_day BIGINT := 86400000000000;
BEGIN
    -- Calculate cutoff timestamp (now - retention_days)
    cutoff_nanos := (EXTRACT(EPOCH FROM NOW()) * 1000000000)::BIGINT 
                    - (retention_days::BIGINT * nanos_per_day);

    -- Find and drop partitions whose upper bound is less than the cutoff
    FOR partition_record IN
        SELECT
            c.relname AS partition_name,
            pg_get_expr(c.relpartbound, c.oid) AS partition_bound
        FROM pg_class c
        JOIN pg_inherits i ON c.oid = i.inhrelid
        JOIN pg_class parent ON i.inhparent = parent.oid
        WHERE parent.relname = 'events'
        AND c.relkind = 'r'
    LOOP
        -- Extract the upper bound from the partition expression
        -- Format: "FOR VALUES FROM (start) TO (end)"
        DECLARE
            upper_bound BIGINT;
        BEGIN
            -- Parse the TO value from the partition bound
            upper_bound := (
                regexp_match(partition_record.partition_bound, 'TO \((\d+)\)')
            )[1]::BIGINT;

            IF upper_bound <= cutoff_nanos THEN
                EXECUTE format('DROP TABLE %I', partition_record.partition_name);
                RAISE NOTICE 'Dropped partition: %', partition_record.partition_name;
                dropped_count := dropped_count + 1;
            END IF;
        EXCEPTION WHEN OTHERS THEN
            RAISE WARNING 'Failed to process partition %: %', 
                partition_record.partition_name, SQLERRM;
        END;
    END LOOP;

    RETURN dropped_count;
END;
$$ LANGUAGE plpgsql;

-- Function to list all event partitions with their date ranges
CREATE OR REPLACE FUNCTION evebox_list_partitions()
RETURNS TABLE (
    partition_name TEXT,
    start_date TIMESTAMP WITH TIME ZONE,
    end_date TIMESTAMP WITH TIME ZONE,
    row_count BIGINT
) AS $$
BEGIN
    RETURN QUERY
    SELECT
        c.relname::TEXT AS partition_name,
        TO_TIMESTAMP(
            (regexp_match(pg_get_expr(c.relpartbound, c.oid), 'FROM \((\d+)\)'))[1]::BIGINT 
            / 1000000000.0
        ) AS start_date,
        TO_TIMESTAMP(
            (regexp_match(pg_get_expr(c.relpartbound, c.oid), 'TO \((\d+)\)'))[1]::BIGINT 
            / 1000000000.0
        ) AS end_date,
        c.reltuples::BIGINT AS row_count
    FROM pg_class c
    JOIN pg_inherits i ON c.oid = i.inhrelid
    JOIN pg_class parent ON i.inhparent = parent.oid
    WHERE parent.relname = 'events'
    AND c.relkind = 'r'
    ORDER BY start_date;
END;
$$ LANGUAGE plpgsql;

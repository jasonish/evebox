-- SPDX-FileCopyrightText: (C) 2020-2025 Jason Ish <jason@codemonkey.net>
-- SPDX-License-Identifier: MIT

-- Enable the pg_trgm extension for trigram support
CREATE EXTENSION IF NOT EXISTS pg_trgm;

-- Events table - partitioned by day for efficient retention management.
-- Partitions are created based on the timestamp field.
CREATE TABLE events (
    -- Timestamp (partition key)
    timestamp TIMESTAMPTZ NOT NULL,

    -- Primary key within partition
    id BIGINT GENERATED ALWAYS AS IDENTITY,

    -- The archived flag
    archived BOOLEAN DEFAULT FALSE,

    -- Escalated/starred flag
    escalated BOOLEAN DEFAULT FALSE,

    -- The actual event as JSONB for efficient querying
    source JSONB,

    -- TSVECTOR generated column for full text search
    source_vector TSVECTOR GENERATED ALWAYS AS (
        jsonb_to_tsvector('simple', source, '["string", "numeric"]')
    ) STORED,

    -- History/comments as JSON array
    history JSONB DEFAULT '[]'::jsonb,

    -- Primary key must include partition key
    PRIMARY KEY (timestamp, id)
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

-- GIN index for full text search on source_vector
CREATE INDEX events_source_vector_idx ON events USING GIN (source_vector);

-- Index on host field to improve performance of sensor list query
CREATE INDEX events_timestamp_host_idx ON events (timestamp, (source->>'host'));

-- Composite index for event_type + timestamp filtering (common in aggregation queries)
CREATE INDEX events_event_type_timestamp_idx 
    ON events ((source->>'event_type'), timestamp DESC);

-- Composite indexes for flow-related aggregations (src_ip, dest_ip, ports, proto)
CREATE INDEX events_flow_src_ip_idx 
    ON events ((source->>'event_type'), timestamp DESC, (source->>'src_ip'))
    WHERE source->>'src_ip' IS NOT NULL;

CREATE INDEX events_flow_dest_ip_idx 
    ON events ((source->>'event_type'), timestamp DESC, (source->>'dest_ip'))
    WHERE source->>'dest_ip' IS NOT NULL;

CREATE INDEX events_flow_src_port_idx 
    ON events ((source->>'event_type'), timestamp DESC, (source->>'src_port'))
    WHERE source->>'src_port' IS NOT NULL;

CREATE INDEX events_flow_dest_port_idx 
    ON events ((source->>'event_type'), timestamp DESC, (source->>'dest_port'))
    WHERE source->>'dest_port' IS NOT NULL;

CREATE INDEX events_flow_proto_idx 
    ON events ((source->>'event_type'), timestamp DESC, (source->>'proto'))
    WHERE source->>'proto' IS NOT NULL;

-- Composite index for DNS query type filtering
CREATE INDEX events_dns_query_type_idx 
    ON events ((source->>'event_type'), (source->'dns'->>'type'), timestamp DESC)
    WHERE source->>'event_type' = 'dns';

-- Function to get partition name for a given timestamp
CREATE OR REPLACE FUNCTION evebox_partition_name(ts TIMESTAMPTZ)
RETURNS TEXT AS $$
BEGIN
    RETURN 'events_' || TO_CHAR(ts, 'YYYY_MM_DD');
END;
$$ LANGUAGE plpgsql IMMUTABLE;

-- Function to create a partition for a given timestamp if it doesn't exist
-- Returns the partition name
CREATE OR REPLACE FUNCTION evebox_ensure_partition(ts TIMESTAMPTZ)
RETURNS TEXT AS $$
DECLARE
    partition_name TEXT;
    day_start TIMESTAMPTZ;
    day_end TIMESTAMPTZ;
BEGIN
    partition_name := evebox_partition_name(ts);
    day_start := DATE_TRUNC('day', ts);
    day_end := day_start + INTERVAL '1 day';

    -- Check if partition exists
    IF NOT EXISTS (
        SELECT 1 FROM pg_class c
        JOIN pg_namespace n ON n.oid = c.relnamespace
        WHERE c.relname = partition_name
        AND n.nspname = current_schema()
    ) THEN
        -- Create the partition
        BEGIN
            EXECUTE format(
                'CREATE TABLE %I PARTITION OF events FOR VALUES FROM (%L) TO (%L)',
                partition_name,
                day_start,
                day_end
            );
            RAISE NOTICE 'Created partition: %', partition_name;
        EXCEPTION WHEN duplicate_table THEN
            -- Partition was created by another process in the meantime
            RAISE NOTICE 'Partition % was created by another process', partition_name;
        END;
    END IF;

    RETURN partition_name;
END;
$$ LANGUAGE plpgsql;

-- Function to drop partitions older than a given number of days
-- Returns the number of partitions dropped
CREATE OR REPLACE FUNCTION evebox_drop_old_partitions(retention_days INTEGER)
RETURNS INTEGER AS $$
DECLARE
    cutoff_ts TIMESTAMPTZ;
    partition_record RECORD;
    dropped_count INTEGER := 0;
BEGIN
    -- Calculate cutoff timestamp (now - retention_days)
    cutoff_ts := NOW() - (retention_days || ' days')::INTERVAL;

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
        DECLARE
            upper_bound TIMESTAMPTZ;
        BEGIN
            -- Parse the TO value from the partition bound
            upper_bound := (
                regexp_match(partition_record.partition_bound, 'TO \(''([^'']+)''\)')
            )[1]::TIMESTAMPTZ;

            IF upper_bound <= cutoff_ts THEN
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
        (regexp_match(pg_get_expr(c.relpartbound, c.oid), 'FROM \(''([^'']+)''\)'))[1]::TIMESTAMPTZ AS start_date,
        (regexp_match(pg_get_expr(c.relpartbound, c.oid), 'TO \(''([^'']+)''\)'))[1]::TIMESTAMPTZ AS end_date,
        c.reltuples::BIGINT AS row_count
    FROM pg_class c
    JOIN pg_inherits i ON c.oid = i.inhrelid
    JOIN pg_class parent ON i.inhparent = parent.oid
    WHERE parent.relname = 'events'
    AND c.relkind = 'r'
    ORDER BY start_date;
END;
$$ LANGUAGE plpgsql;

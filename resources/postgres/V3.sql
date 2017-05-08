-- Create/update the indexes on the events_YYYYMMDD tables.
CREATE OR REPLACE FUNCTION update_events_table_indexes(TEXT)
  RETURNS VOID AS $$
DECLARE
  table_name TEXT = $1;
BEGIN

  EXECUTE format(
      'create index if not exists %s_timestamp_index on %s (timestamp)',
      table_name, table_name);

  EXECUTE format(
      'create index if not exists %s_archived_index on %s (archived)',
      table_name, table_name);

  EXECUTE format(
      'create index if not exists %s_escalated_index on %s (escalated)',
      table_name, table_name);

  EXECUTE format(
      'create index if not exists %s_metadata_index on %s using gin (metadata)',
      table_name, table_name);

END;
$$ LANGUAGE plpgsql;

-- Create/update the indexes on the events_source_YYYYMMDD tables.
CREATE OR REPLACE FUNCTION update_events_source_table_indexes(TEXT)
  RETURNS VOID AS $$
DECLARE
  table_name TEXT = $1;
BEGIN

  -- Index on timestamp.
  EXECUTE format(
      'create index if not exists %s_timestamp_index on %s (timestamp)',
      table_name, table_name);

  -- Index on source.event_type.
  EXECUTE format(
      'create index if not exists %s_event_type_index on %s ((source->>''event_type''))',
      table_name, table_name);

  -- Index on source.alert.signature_id.
  EXECUTE format(
      'create index if not exists %s_alert_signature_id_index ' ||
      'on %s (((source->''alert''->>''signature_id'')::bigint))' ||
      'where source->>''event_type'' = ''alert''',
      table_name, table_name);

  -- Index on source.src_ip.
  EXECUTE format(
      'create index if not exists %s_src_ip_index on %s (((source->>''src_ip'')::inet))',
      table_name, table_name);

  -- Index on source.dest_ip.
  EXECUTE format(
      'create index if not exists %s_dest_ip_index on %s (((source->>''dest_ip'')::inet))',
      table_name, table_name);

END;
$$ LANGUAGE plpgsql;

-- For all event tables run the index functions to add new indexes.
CREATE OR REPLACE FUNCTION update_indexes()
  RETURNS VOID AS $$
DECLARE
  loop_table RECORD;
BEGIN

  FOR loop_table IN SELECT table_name
                    FROM information_schema.tables
                    WHERE table_name ~ '^events_\d{8}$'
                    ORDER BY table_name
  LOOP
    PERFORM update_events_table_indexes(loop_table.table_name);
  END LOOP;

  FOR loop_table IN SELECT table_name
                    FROM information_schema.tables
                    WHERE table_name ~ '^events_source_\d{8}$'
                    ORDER BY table_name
  LOOP
    PERFORM update_events_source_table_indexes(loop_table.table_name);
  END LOOP;

END;
$$ LANGUAGE plpgsql;

CREATE OR REPLACE FUNCTION evebox_create_events_table(TEXT)
  RETURNS BOOLEAN AS
$$
DECLARE
  DATE       TEXT := $1;
  table_name TEXT := 'events_' || DATE;
  end_date   TEXT;
BEGIN

  -- Check if table exists, and just return if so.
  PERFORM relname
  FROM pg_class
  WHERE relname = table_name;
  IF found
  THEN RETURN TRUE;
  END IF;

  SELECT INTO end_date DATE(DATE) + '1 day' :: INTERVAL;

  EXECUTE format(
      'create table events_%s (primary key (uuid), check (timestamp >= %L AND timestamp < %L)) inherits (events)',
      DATE, DATE || '+00', end_date || '+00');

  PERFORM update_events_table_indexes('events_' || DATE);

  EXECUTE format(
      'create table events_source_%s ' ||
      '(primary key (uuid), check (timestamp >= %L AND timestamp < %L)) ' ||
      'inherits (events_source)',
      DATE, DATE || '+00', end_date || '+00');

  PERFORM update_events_source_table_indexes('events_source_' || DATE);

  RETURN TRUE;
END;
$$ LANGUAGE plpgsql;

-- Update the indexes.
SELECT update_indexes();

INSERT INTO schema (VERSION, TIMESTAMP) VALUES (
  3, NOW());

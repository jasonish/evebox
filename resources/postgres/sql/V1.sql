DROP TABLE IF EXISTS events_master;

CREATE TABLE events_master (
  uuid      UUID                     NOT NULL PRIMARY KEY,
  timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
  source    JSONB
);

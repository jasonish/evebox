DROP TABLE IF EXISTS events CASCADE;
DROP TABLE IF EXISTS events_source CASCADE;

CREATE TABLE events (
  uuid      UUID                     NOT NULL PRIMARY KEY,
  timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
  archived  BOOLEAN DEFAULT FALSE,
  escalated BOOLEAN DEFAULT FALSE
);

CREATE TABLE events_source (
  uuid      UUID                     NOT NULL PRIMARY KEY,
  timestamp TIMESTAMP WITH TIME ZONE NOT NULL,
  source    JSONB
);

INSERT INTO schema (VERSION, TIMESTAMP) VALUES (
  1, NOW());
-- Generic key/value store for config data.
CREATE TABLE kv (
  key STRING UNIQUE NOT NULL,
  value JSON NOT NULL
);

CREATE INDEX kv_key_index ON kv (key);

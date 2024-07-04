CREATE TABLE ja4db (
  fingerprint STRING UNIQUE,
  data JSON NOT NULL
);

CREATE INDEX ja4db_fingerprint_index ON ja4db (fingerprint);

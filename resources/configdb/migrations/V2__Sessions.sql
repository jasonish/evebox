DROP TABLE IF EXISTS sessions;

CREATE TABLE sessions (
  rowid INTEGER PRIMARY KEY,
  token STRING UNIQUE NOT NULL,
  uuid STRING NOT NULL,
  expires_at INTEGER NOT NULL,
  FOREIGN KEY (uuid) REFERENCES users(uuid)
);

CREATE INDEX sessions_token_index ON sessions (token);

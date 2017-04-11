DROP TABLE IF EXISTS users;

CREATE TABLE users (
  uuid      string UNIQUE NOT NULL,
  username  string UNIQUE NOT NULL,
  fullname  string,
  email     string UNIQUE,

  -- Password hash.
  password  string,

  github_id INTEGER UNIQUE
);

CREATE INDEX users_username_index
  ON users (username);

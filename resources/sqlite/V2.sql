-- Change the events_fts table to be indexed by the rowid. This requires the
-- old table to be destroyed and recreated. But makes deleting much much
-- faster. The new table is also contentless, so should be more space
-- efficient as well.
DROP TABLE events_fts;

CREATE VIRTUAL TABLE events_fts USING fts5(source, content = '');

INSERT INTO events_fts (rowid, source) SELECT
                                         rowid,
                                         source
                                       FROM events;

-- Deleting from a content-less table requires a trigger like this.
CREATE TRIGGER events_delete
AFTER DELETE ON events
BEGIN
  INSERT INTO events_fts (events_fts, rowid) VALUES ('delete', old.rowid);
END;
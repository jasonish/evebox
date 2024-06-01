ALTER TABLE events
      ADD COLUMN history JSON
      default '[]';

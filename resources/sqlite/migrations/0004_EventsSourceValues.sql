-- A textual column that will only contain values extracted from the
-- JSON without field names.  Known base64 values will not be
-- included. This is the data that will be indexed with full text
-- search.
ALTER TABLE events ADD source_values TEXT;

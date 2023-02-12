-- These don't appear to be the best indexes for the inbox and
-- escalated views.
DROP INDEX IF EXISTS events_event_type_archived;
DROP INDEX IF EXISTS events_escalated_view_index;

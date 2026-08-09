UPDATE filters
SET comment = COALESCE(comment, json_extract(filter, '$.comment')),
    filter = json_object(
        'action', 'archive',
        'conditions', json(
            '[' ||
            CASE WHEN json_type(filter, '$.sensor') IS NOT NULL THEN
                json_object(
                    'field', 'host',
                    'op', 'eq',
                    'value', json_extract(filter, '$.sensor')) || ','
            ELSE '' END ||
            CASE WHEN json_type(filter, '$.src_ip') IS NOT NULL THEN
                json_object(
                    'field', 'src_ip',
                    'op', 'eq',
                    'value', json_extract(filter, '$.src_ip')) || ','
            ELSE '' END ||
            CASE WHEN json_type(filter, '$.dest_ip') IS NOT NULL THEN
                json_object(
                    'field', 'dest_ip',
                    'op', 'eq',
                    'value', json_extract(filter, '$.dest_ip')) || ','
            ELSE '' END ||
            json_object(
                'field', 'alert.signature_id',
                'op', 'eq',
                'value', json_extract(filter, '$.signature_id')) ||
            ']'))
WHERE json_type(filter, '$.conditions') IS NULL
  AND json_type(filter, '$.signature_id') IS NOT NULL;

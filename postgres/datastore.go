/* Copyright (c) 2016 Jason Ish
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 *
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED ``AS IS'' AND ANY EXPRESS OR IMPLIED
 * WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * DISCLAIMED. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY DIRECT,
 * INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
 * (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING
 * IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
 * POSSIBILITY OF SUCH DAMAGE.
 */

package postgres

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/elasticsearch"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/sqlite"
	"github.com/jasonish/evebox/util"
	"github.com/pkg/errors"
	"regexp"
	"strings"
	"time"
)

type PgDatastore struct {
	core.UnimplementedDatastore
	pg *PgDB
}

func NewPgDatastore(pg *PgDB) *PgDatastore {
	return &PgDatastore{
		pg: pg,
	}
}

func (d *PgDatastore) GetEveEventSink() core.EveEventSink {
	return NewPgEventIndexer(d.pg)
}

func (d *PgDatastore) GetEventById(eventId string) (map[string]interface{}, error) {
	sqlTemplate := `
SELECT
  e.uuid, e.archived, e.escalated, e.metadata->>'history', s.source
FROM
  events as e, events_source as s
WHERE
  e.uuid = $1 AND e.uuid = s.uuid`
	startTime := time.Now()
	rows, err := d.pg.Query(sqlTemplate, eventId)
	if err != nil {
		return nil, errors.Wrap(err, "query failed")
	}
	log.Info("Query time for get event by ID: %v", time.Now().Sub(startTime))
	for rows.Next() {
		var eventId string
		var archived bool
		var escalated bool
		var rawHistory sql.NullString
		var rawSource string
		err = rows.Scan(&eventId, &archived, &escalated, &rawHistory, &rawSource)
		if err != nil {
			return nil, errors.Wrap(err, "failed to scan result")
		}
		source, err := eve.NewEveEventFromString(rawSource)
		if err != nil {
			return nil, errors.Wrap(err, "failed to parse event")
		}

		if archived {
			source.AddTag("archived")
			source.AddTag("evebox.archived")
		}

		if escalated {
			source.AddTag("escalated")
		}

		if rawHistory.Valid {
			var history []interface{}
			if err := json.Unmarshal([]byte(rawHistory.String), &history); err != nil {
				log.Error("Failed to encode history: %v", err)
			} else {
				source["evebox"] = map[string]interface{}{
					"history": history,
				}
			}
		}

		return map[string]interface{}{
			"_id":     eventId,
			"_source": source,
		}, nil
	}
	return nil, nil
}

func (d *PgDatastore) AlertQuery(options core.AlertQueryOptions) ([]core.AlertGroup, error) {
	log.Info("Must have tags: %v", options.MustHaveTags)
	log.Info("Must not have tags: %v", options.MustNotHaveTags)
	sqlTemplate := `SELECT
DISTINCT ON (maxts, grouped.sig_id, grouped.src_ip, grouped.dest_ip)
  grouped.count as count,
  grouped.escalated_count as escalated_count,
  events.uuid as uuid,
  grouped.maxts AS maxts,
  grouped.mints as mints,
  events_source.source,
  grouped.archived_count as archived_count,
  events.archived as archived,
  metadata->>'history' as history
FROM (
       SELECT
         count(events_source.source -> 'alert' ->> 'signature_id')      AS count,
         count(CASE WHEN events.escalated = true
           THEN 1 END)                                      AS escalated_count,
         count(CASE WHEN events.archived = true
           THEN 1 END)                                      AS archived_count,
         max(events.timestamp)                              AS maxts,
         min(events.timestamp)                              AS mints,
         (events_source.source -> 'alert' ->> 'signature_id') :: BIGINT AS sig_id,
         (events_source.source ->> 'src_ip') :: INET                    AS src_ip,
         (events_source.source ->> 'dest_ip') :: INET                   AS dest_ip
       FROM
         events, events_source
       WHERE
         events.uuid = events_source.uuid
         AND events_source.source ->> 'event_type' = 'alert'
         %%AND_EVENTS_ARCHIVED%%
         %%AND_EVENTS_ESCALATED%%
         %%AND_EVENTS_SOURCE_MINTS%%
         %%AND_EVENTS_MINTS%%
         %%QUERYSTRING%%
       GROUP BY (events_source.source -> 'alert' ->> 'signature_id') :: BIGINT,
         (events_source.source ->> 'src_ip') :: INET,
         (events_source.source ->> 'dest_ip') :: INET
     ) AS grouped
  JOIN events_source
    ON events_source.timestamp = grouped.maxts
       AND events_source.source ->> 'event_type' = 'alert'
       AND (events_source.source -> 'alert' ->> 'signature_id') :: BIGINT =
           grouped.sig_id
       AND (events_source.source ->> 'src_ip') :: INET = grouped.src_ip
       AND (events_source.source ->> 'dest_ip') :: INET = grouped.dest_ip
       %%AND_EVENTS_SOURCE_MINTS%%
  , events
WHERE
  events.uuid = events_source.uuid
  %%AND_EVENTS_ARCHIVED%%
  %%AND_EVENTS_ESCALATED%%
  %%AND_EVENTS_MINTS%%
ORDER BY maxts DESC;
`
	now := time.Now()

	args := []interface{}{}

	if options.TimeRange != "" {
		duration, err := time.ParseDuration(options.TimeRange)
		if err != nil {
			return nil, errors.Wrap(err, "failed to parse time range")
		}
		minTs := now.Add(duration * -1)

		sqlTemplate = strings.Replace(sqlTemplate,
			"%%AND_EVENTS_SOURCE_MINTS%%",
			fmt.Sprintf("AND events_source.timestamp >= $%d::timestamptz", len(args)+1),
			-1)

		sqlTemplate = strings.Replace(sqlTemplate,
			"%%AND_EVENTS_MINTS%%",
			fmt.Sprintf("AND events.timestamp >= $%d::timestamptz", len(args)+1),
			-1)

		args = append(args, minTs)
	}

	for _, tag := range options.MustNotHaveTags {
		switch tag {
		case "archived":
			sqlTemplate = strings.Replace(sqlTemplate,
				"%%AND_EVENTS_ARCHIVED%%",
				"AND events.archived = false", -1)
		default:
			log.Warning("Unsupported must-not-have-tag: %s", tag)
		}
	}

	for _, tag := range options.MustHaveTags {
		switch tag {
		case "escalated":
			sqlTemplate = strings.Replace(sqlTemplate,
				"%%AND_EVENTS_ESCALATED%%",
				"AND events.escalated = true",
				-1)
		default:
			log.Warning("Unsupported must-not-have-tag: %s", tag)
		}
	}

	if options.QueryString != "" {
		filters := []string{}
		parseQueryString(options.QueryString, &filters, &args)
		if len(filters) > 0 {
			where := fmt.Sprintf("AND %s", strings.Join(filters, " AND "))
			sqlTemplate = strings.Replace(sqlTemplate, "%%QUERYSTRING%%",
				where, -1)
		}
	}

	re := regexp.MustCompile("%%[A-Z_]+%%")
	sqlTemplate = re.ReplaceAllLiteralString(sqlTemplate, " ")

	qStart := time.Now()

	rows, err := d.pg.Query(sqlTemplate, args...)
	if err != nil {
		log.Error("Alert query failed: %v", err)
		return nil, errors.Wrap(err, "query error")
	}
	log.Info("Query time: %v", time.Now().Sub(qStart))

	alerts := []core.AlertGroup{}

	for rows.Next() {
		var count int64
		var escalatedCount int64
		var eventId string
		var maxTs time.Time
		var minTs time.Time
		var rawSource string
		var archivedCount int64
		var archived bool
		var rawHistory sql.NullString
		err = rows.Scan(&count,
			&escalatedCount,
			&eventId,
			&maxTs,
			&minTs,
			&rawSource,
			&archivedCount,
			&archived,
			&rawHistory)
		if err != nil {
			log.Error("scan: %v", err)
			continue
		}

		source, err := eve.NewEveEventFromString(rawSource)
		if err != nil {
			log.Error("Failed to convert raw event to event: %v", err)
			continue
		}

		if rawHistory.Valid {
			var history []interface{}
			if err := json.Unmarshal([]byte(rawHistory.String), &history); err != nil {
				log.Error("Failed to encode history: %v", err)
			} else {
				source["evebox"] = map[string]interface{}{
					"history": history,
				}
			}
		}

		if archived {
			source.AddTag("archived")
			source.AddTag("evebox.archived")
		}

		alert := core.AlertGroup{
			Count: count,
			Event: map[string]interface{}{
				"_id":     eventId,
				"_source": source,
			},
			MinTs:          eve.FormatTimestampUTC(minTs),
			MaxTs:          eve.FormatTimestampUTC(maxTs),
			EscalatedCount: escalatedCount,
		}

		alerts = append(alerts, alert)
	}
	rows.Close()

	return alerts, nil
}

func (d *PgDatastore) FindFlow(flowId uint64, proto string, timestamp string,
	srcIp string, destIp string) (interface{}, error) {
	sqlTemplate := `select s.uuid, s.source
from events_source as s
where
s.source #>> '{event_type}' = 'flow'
and s.source #>> '{flow_id}' = $1
and (s.source #>> '{src_ip}' = $2 OR s.source #>> '{dest_ip}' = $2)
and (s.source #>> '{dest_ip}' = $3 OR s.source #>> '{src_ip}' = $3)
and lower(s.source #>> '{proto}') = $4
and cast(s.source #>> '{flow,start}' AS timestamptz) <= $5
and cast(s.source #>> '{flow,end}' AS timestamptz) >= $5
`
	ts, err := eve.ParseTimestamp(timestamp)
	if err != nil {
		return nil, errors.Wrap(err, "failed to parse timestamp")
	}

	rows, err := d.pg.Query(sqlTemplate, flowId, srcIp, destIp,
		strings.ToLower(proto), ts)
	if err != nil {
		return nil, errors.Wrap(err, "query failed")
	}
	for rows.Next() {
		var eventId string
		var rawSource string
		err = rows.Scan(&eventId, &rawSource)
		if err != nil {
			return nil, errors.Wrap(err, "failed to scan result")
		}

		source, err := eve.NewEveEventFromString(rawSource)
		if err != nil {
			return nil, errors.Wrap(err, "failed to parse event")
		}

		return []interface{}{
			map[string]interface{}{
				"_id":     eventId,
				"_source": source,
			},
		}, nil
	}
	return nil, nil
}

func (d *PgDatastore) ArchiveAlertGroup(p core.AlertGroupQueryParams, user core.User) (err error) {
	var maxTime time.Time
	if !p.MaxTimestamp.IsZero() {
		maxTime = p.MaxTimestamp
	} else {
		maxTime = time.Now()
	}

	var minTime time.Time
	if !p.MinTimestamp.IsZero() {
		minTime = p.MinTimestamp
	}

	history := elasticsearch.HistoryEntry{
		Timestamp: elasticsearch.FormatTimestampUTC(time.Now()),
		Username:  user.Username,
		Action:    elasticsearch.ACTION_ARCHIVED,
	}

	sqlTemplate := `update events
set
  archived = true,
  metadata = jsonb_set(
    metadata,
    '{"history"}',
    case when metadata->'history' is null then '[]'::jsonb
      else metadata->'history' end || $6::jsonb
    )
where
  archived = false
  and timestamp <= $4::timestamptz
  and timestamp >= $5::timestamptz
  and uuid in (
    select uuid from events_source
    where
      source->>'event_type' = 'alert'
      AND (source->>'src_ip')::inet = $1::inet
      AND (source->>'dest_ip')::inet = $2::inet
      AND (source->'alert'->>'signature_id')::bigint = $3
      AND timestamp <= $4::timestamptz
      AND timestamp >= $5::timestamptz
  )
`

	args := []interface{}{
		p.SrcIP,
		p.DstIP,
		p.SignatureID,
		maxTime,
		minTime,
		util.ToJson(history),
	}

	qstart := time.Now()
	_, err = d.pg.Exec(sqlTemplate, args...)
	log.Info("Update time: %v", time.Now().Sub(qstart))
	if err != nil {
		return errors.Wrap(err, "query failed")
	}
	return nil
}

func (d *PgDatastore) EscalateAlertGroup(p core.AlertGroupQueryParams, user core.User) (err error) {
	var maxTime time.Time
	if p.MaxTimestamp.IsZero() {
		maxTime = time.Now()
	} else {
		maxTime = p.MaxTimestamp
	}

	var minTime time.Time
	if !p.MinTimestamp.IsZero() {
		minTime = p.MinTimestamp
	}

	sqlTemplate := `
update events
set
  escalated = true,
  metadata = jsonb_set(
    metadata,
    '{"history"}',
    case when metadata->'history' is null then '[]'::jsonb
      else metadata->'history' end || $6::jsonb
    )
where
  escalated = false
  and timestamp <= $4
  and timestamp >= $5
  and uuid in (
    select uuid from events_source
    where
      source->>'event_type' = 'alert'
      AND (source->>'src_ip')::inet = $1::inet
      AND (source->>'dest_ip')::inet = $2::inet
      AND (source->'alert'->>'signature_id')::bigint = $3
      AND timestamp <= $4
      AND timestamp >= $5
    )
`

	history := elasticsearch.HistoryEntry{
		Timestamp: eve.FormatTimestampUTC(time.Now()),
		Username:  user.Username,
		Action:    elasticsearch.ACTION_ESCALATED,
	}

	qstart := time.Now()
	_, err = d.pg.Exec(sqlTemplate,
		p.SrcIP,
		p.DstIP,
		p.SignatureID,
		maxTime,
		minTime,
		util.ToJson(history))
	log.Info("Update time: %v", time.Now().Sub(qstart))
	if err != nil {
		return errors.Wrap(err, "query failed")
	}
	return nil
}

func (d *PgDatastore) DeEscalateAlertGroup(p core.AlertGroupQueryParams, user core.User) (err error) {
	var maxTime time.Time
	if !p.MaxTimestamp.IsZero() {
		maxTime = p.MaxTimestamp
	} else {
		maxTime = time.Now()
	}

	var minTime time.Time
	if !p.MinTimestamp.IsZero() {
		minTime = p.MinTimestamp
	}

	sqlTemplate := `
update events
set
  escalated = false,
  metadata = jsonb_set(
    metadata,
    '{"history"}',
    case when metadata->'history' is null then '[]'::jsonb
      else metadata->'history' end || $6::jsonb
    )
where
  escalated = true
  and timestamp <= $4
  and timestamp >= $5
  and uuid in (
    select uuid from events_source
    where
      source->>'event_type' = 'alert'
      AND (source->>'src_ip')::inet = $1
      AND (source->>'dest_ip')::inet = $2
      AND (source->'alert'->>'signature_id')::bigint = $3
      AND timestamp <= $4
      AND timestamp >= $5
    )
`

	history := elasticsearch.HistoryEntry{
		Timestamp: eve.FormatTimestampUTC(time.Now()),
		Username:  user.Username,
		Action:    elasticsearch.ACTION_DEESCALATED,
	}

	qstart := time.Now()
	_, err = d.pg.Exec(sqlTemplate,
		p.SrcIP,
		p.DstIP,
		p.SignatureID,
		maxTime,
		minTime,
		util.ToJson(history))
	log.Info("Update time: %v", time.Now().Sub(qstart))
	if err != nil {
		return errors.Wrap(err, "query failed")
	}
	return nil
}

func (s *PgDatastore) EventQuery(options core.EventQueryOptions) (interface{}, error) {
	sqlTemplate := `
select
  events_source.uuid,
  events_source.source,
  events.archived
from events_source, events
where
  events_source.source->>'event_type' != 'stats'
  AND events_source.uuid = events.uuid
  %%AND_EVENT_TYPE%%
  %%QUERYSTRING%%
order by events_source.timestamp %%ORDER%%
limit 500
	`

	args := []interface{}{}

	if options.EventType != "" {
		sqlTemplate = strings.Replace(sqlTemplate,
			"%%AND_EVENT_TYPE%%",
			fmt.Sprintf("AND events_source.source->>'event_type' = $%d", len(args)+1),
			-1)
		args = append(args, options.EventType)
	}

	if options.QueryString != "" {
		filters := []string{}
		parseQueryString(options.QueryString, &filters, &args)
		if len(filters) > 0 {
			where := fmt.Sprintf("AND %s", strings.Join(filters, " AND "))
			sqlTemplate = strings.Replace(sqlTemplate, "%%QUERYSTRING%%",
				where, -1)
		}
	}

	if options.Order == "asc" {
		sqlTemplate = strings.Replace(sqlTemplate, "%%ORDER%%", "asc", -1)
	} else {
		sqlTemplate = strings.Replace(sqlTemplate, "%%ORDER%%", "desc", -1)
	}

	re := regexp.MustCompile("%%[A-Z_]+%%")
	sqlTemplate = re.ReplaceAllLiteralString(sqlTemplate, " ")

	events := []interface{}{}

	rows, err := s.pg.Query(sqlTemplate, args...)
	if err != nil {
		log.Error("query failed: %v", err)
		return nil, errors.Wrap(err, "query failed")
	}
	for rows.Next() {
		var eventId string
		var rawSource string
		var archived bool
		if err := rows.Scan(&eventId, &rawSource, &archived); err != nil {
			log.Error("Failed to scan raw: %v", err)
			continue
		}
		source, err := eve.NewEveEventFromString(rawSource)
		if err != nil {
			log.Error("Failed to convert source to event: %v", source)
			continue
		}

		if archived {
			source.AddTag("archived")
			source.AddTag("evebox.archived")
		}

		events = append(events, map[string]interface{}{
			"_id":     eventId,
			"_source": source,
		})
	}

	return map[string]interface{}{
		"data": events,
	}, nil
}

func dumpQuery(query string, args []interface{}) {
	for i, arg := range args {
		placeholder := fmt.Sprintf("$%d", i+1)
		switch arg := arg.(type) {
		case time.Time:
			query = strings.Replace(query, placeholder,
				fmt.Sprintf("'%s'", eve.FormatTimestamp(arg)),
				-1)
		case uint64:
			query = strings.Replace(query, placeholder,
				fmt.Sprintf("%d", arg), -1)
		case int64:
			query = strings.Replace(query, placeholder,
				fmt.Sprintf("%d", arg), -1)
		case string:
			query = strings.Replace(query, placeholder,
				fmt.Sprintf("'%s'", arg), -1)
		default:
			query = strings.Replace(query,
				fmt.Sprintf("$%d", i+1), fmt.Sprintf("$q", arg), -1)
		}
	}
	log.Println(strings.Replace(query, "\n", " ", -1))
}

// Parse a query string for a PostgreSQL search.
//
// Supported keywords and args:
//     src_ip:ADDR
//     dest_ip:ADDR
//     was:escalated
//     is:escalated
//     has:comment
//     comment:STRING
//
// Any other key/value pair will be constructed into a query doing an 'ILIKE'
// on the JSON field of the event.
//
// An argument with no key will be done with an 'ILIKE' on the text representation
// of the JSON event.
func parseQueryString(queryString string, filters *[]string, args *[]interface{}) {
	parser := sqlite.NewQueryStringParser(queryString)
	for {
		key, val := parser.Next()

		if key != "" && val != "" {
			if key == "src_ip" {
				filter := fmt.Sprintf("(events_source.source->>'src_ip')::inet = $%d::inet", len(*args)+1)
				*filters = append(*filters, filter)
				*args = append(*args, val)
				log.Debug("Adding query filter %s with arg %v", filter, val)
			} else if key == "dest_ip" {
				filter := fmt.Sprintf("(events_source.source->>'dest_ip')::inet = $%d::inet", len(*args)+1)
				*filters = append(*filters, filter)
				*args = append(*args, val)
				log.Debug("Adding query filter %s with arg %v", filter, val)
			} else if key == "was" && val == "escalated" {
				*filters = append(*filters,
					`events.metadata->'history' @> '[{"action": "escalated"}]'::jsonb`)
			} else if key == "is" && val == "escalated" {
				*filters = append(*filters, `events.escalated = true`)
			} else if key == "has" && val == "comment" {
				*filters = append(*filters,
					`events.metadata @> '{"history": [{"action": "comment"}]}'::jsonb`)
			} else if key == "comment" {
				*filters = append(*filters,
					fmt.Sprintf(`
					    events.metadata @> '{"history": [{"action": "comment"}]}'
					    AND (events.metadata->'history')::text ILIKE $%d`, len(*args)+1))
				*args = append(*args, fmt.Sprintf("%%%s%%", val))
			} else {
				path := strings.Replace(key, ".", ",", -1)
				filter := fmt.Sprintf("events_source.source #>> '{%s}' ILIKE $%d", path, len(*args)+1)
				*filters = append(*filters, filter)
				*args = append(*args, val)
				log.Debug("Adding query filter %s with arg %v", filter, val)
			}
		} else if val != "" {
			*filters = append(*filters, fmt.Sprintf("events_source.source::text ILIKE $%d", len(*args)+1))
			arg := fmt.Sprintf("%%%s%%", val)
			*args = append(*args, arg)
			log.Debug("Adding query filter %s (%v)", (*filters)[len(*filters)-1], arg)
		} else if key == "" && val == "" {
			break
		}
	}
}

func (d *PgDatastore) CommentOnAlertGroup(p core.AlertGroupQueryParams, user core.User, comment string) (err error) {

	var maxTime time.Time
	if !p.MaxTimestamp.IsZero() {
		maxTime = p.MaxTimestamp
	} else {
		maxTime = time.Now()
	}

	var minTime time.Time
	if !p.MinTimestamp.IsZero() {
		minTime = p.MinTimestamp
	}

	sqlTemplate := `
update events
set
  metadata = jsonb_set(
    metadata,
    '{"history"}',
    case when metadata->'history' is null then '[]'::jsonb
      else metadata->'history' end || $6::jsonb
    )
where
  timestamp <= $4
  and timestamp >= $5
  and uuid in (
    select uuid from events_source
    where
      source->>'event_type' = 'alert'
      AND (source->>'src_ip')::inet = $1
      AND (source->>'dest_ip')::inet = $2
      AND (source->'alert'->>'signature_id')::bigint = $3
      AND timestamp <= $4
      AND timestamp >= $5
    )
`

	history := elasticsearch.HistoryEntry{
		Timestamp: elasticsearch.FormatTimestampUTC(time.Now()),
		Username:  user.Username,
		Action:    elasticsearch.ACTION_COMMENT,
		Comment:   comment,
	}

	qstart := time.Now()
	_, err = d.pg.Exec(sqlTemplate,
		p.SrcIP,
		p.DstIP,
		p.SignatureID,
		maxTime,
		minTime,
		util.ToJson(history))
	log.Info("Update time: %v", time.Now().Sub(qstart))
	if err != nil {
		return errors.Wrap(err, "query failed")
	}
	return nil
}

func (d *PgDatastore) CommentOnEventId(eventId string, user core.User, comment string) error {

	history := elasticsearch.HistoryEntry{
		Timestamp: elasticsearch.FormatTimestampUTC(time.Now()),
		Username:  user.Username,
		Action:    elasticsearch.ACTION_COMMENT,
		Comment:   comment,
	}

	sqlTemplate := `update events
set
  metadata = jsonb_set(
    metadata,
    '{"history"}',
    case when metadata->'history' is null then '[]'::jsonb
      else metadata->'history' end || $1::jsonb
    )
where
  uuid = $2
`
	_, err := d.pg.Exec(sqlTemplate, util.ToJson(history), eventId)
	if err != nil {
		return errors.Wrap(err, "update query failed")
	}

	return nil
}

// +build cgo

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

package sqlite

import (
	"fmt"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/util"
	"github.com/pkg/errors"
	"strconv"
	"strings"
	"time"
)

type DataStore struct {
	core.UnimplementedDatastore
	db *SqliteService
}

func NewDataStore(db *SqliteService) *DataStore {
	return &DataStore{
		db: db,
	}
}

func (d *DataStore) GetEveEventSink() core.EveEventSink {
	return NewSqliteIndexer(d.db)
}

func (s *DataStore) GetEventById(id string) (map[string]interface{}, error) {
	builder := SqlBuilder{}
	builder.Select("source")
	builder.From("events")
	builder.WhereEquals("rowid", id)

	tx, err := s.db.GetTx()
	if err != nil {
		log.Error("%v", err)
		return nil, err
	}
	defer tx.Commit()

	rows, err := tx.Query(builder.Build(), builder.Args()...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	for rows.Next() {
		var rawEvent []byte
		err = rows.Scan(&rawEvent)
		if err != nil {
			return nil, err
		}
		event, err := eve.NewEveEventFromBytes(rawEvent)
		if err != nil {
			return nil, err
		}

		wrapper := map[string]interface{}{
			"_id":     id,
			"_source": event,
		}

		return wrapper, nil
	}

	return nil, nil
}

func (s *DataStore) AlertQuery(options core.AlertQueryOptions) ([]core.AlertGroup, error) {

	query := `
SELECT b.count,
  a.rowid as id,
  b.mints as mints,
  b.escalated_count,
  a.archived,
  a.source
FROM events a
  INNER JOIN
  (
    SELECT
      events.rowid,
      count(json_extract(events.source, '$.alert.signature_id')) as count,
      min(timestamp) as mints,
      max(timestamp) AS maxts,
      sum(escalated) as escalated_count
    %FROM%
    %WHERE%
    GROUP BY
      json_extract(events.source, '$.alert.signature_id'),
      json_extract(events.source, '$.src_ip'),
      json_extract(events.source, '$.dest_ip')
  ) AS b
WHERE a.rowid = b.rowid AND a.timestamp = b.maxts
ORDER BY timestamp DESC`

	builder := SqlBuilder{}
	builder.From("events")

	builder.WhereEquals("json_extract(events.source, '$.event_type')", "alert")

	if util.StringSliceContains(options.MustHaveTags, "archived") {
		builder.WhereEquals("archived", 1)
	}

	if util.StringSliceContains(options.MustNotHaveTags, "archived") {
		builder.WhereEquals("archived", 0)
	}

	if util.StringSliceContains(options.MustHaveTags, "escalated") {
		builder.WhereEquals("escalated", 1)
	}

	if options.QueryString != "" {
		parseQueryString(&builder, options.QueryString, "events")
	}

	now := time.Now()

	if options.TimeRange != "" {
		duration, err := time.ParseDuration(options.TimeRange)
		if err != nil {
			return nil, errors.Wrap(err, "failed to parse duration string)")
		}
		minTs := now.Add(duration * -1)
		builder.WhereGte("timestamp", minTs.UnixNano())
	} else {
		if !options.MinTs.IsZero() {
			builder.WhereGte("timestamp", options.MinTs.UnixNano())
		}
		if !options.MaxTs.IsZero() {
			builder.WhereLte("timestamp", options.MaxTs.UnixNano())
		}
	}

	query = strings.Replace(query, "%WHERE%", builder.BuildWhere(), 1)
	query = strings.Replace(query, "%FROM%", builder.BuildFrom(), 1)

	tx, err := s.db.GetTx()
	if err != nil {
		log.Error("%v", err)
		return nil, err
	}
	defer tx.Commit()
	queryStart := time.Now()
	rows, err := tx.Query(query, builder.args...)
	if err != nil {
		log.Error("%v", err)
		return nil, err
	}
	defer rows.Close()

	alerts := make([]core.AlertGroup, 0)

	for rows.Next() {
		var count int64
		var minTsNanos int64
		var id int64
		var escalated int64
		var archived int8
		var rawEvent []byte

		err = rows.Scan(&count,
			&id,
			&minTsNanos,
			&escalated,
			&archived,
			&rawEvent)
		if err != nil {
			log.Error("%v", err)
			return nil, err
		}

		event, err := eve.NewEveEventFromBytes(rawEvent)
		if err != nil {
			return nil, err
		}

		if archived > 0 {
			event["tags"] = append(event["tags"].([]interface{}),
				"archived")
		}

		alert := core.AlertGroup{
			Count: count,
			Event: map[string]interface{}{
				"_id":     id,
				"_source": event,
			},
			MinTs:          eve.FormatTimestampUTC(time.Unix(0, minTsNanos)),
			MaxTs:          eve.FormatTimestampUTC(event.Timestamp()),
			EscalatedCount: escalated,
		}

		alerts = append(alerts, alert)
	}
	log.Debug("Alert query execution time: %v", time.Now().Sub(queryStart))

	return alerts, nil
}

func (s *DataStore) ArchiveAlertGroup(p core.AlertGroupQueryParams, user core.User) error {

	b := SqlBuilder{}

	b.Select("rowid")
	b.From("events")
	b.WhereEquals("archived", 0)
	b.WhereEquals(
		"json_extract(events.source, '$.alert.signature_id')",
		p.SignatureID)
	b.WhereEquals(
		"json_extract(events.source, '$.src_ip')",
		p.SrcIP)
	b.WhereEquals(
		"json_extract(events.source, '$.dest_ip')",
		p.DstIP)
	if !p.MinTimestamp.IsZero() {
		b.WhereGte("timestamp", p.MinTimestamp.UnixNano())
	}
	if !p.MaxTimestamp.IsZero() {
		b.WhereLte("timestamp", p.MaxTimestamp.UnixNano())
	}

	// TODO - query string

	query := fmt.Sprintf("UPDATE events SET archived = 1 WHERE rowid IN (%s)", b.Build())

	tx, err := s.db.GetTx()
	if err != nil {
		log.Error("%v", err)
	}
	defer tx.Commit()

	start := time.Now()
	r, err := tx.Exec(query, b.args...)
	if err != nil {
		log.Error("error archiving alerts: %v", err)
		return err
	}
	duration := time.Now().Sub(start)
	count, err := r.RowsAffected()
	if err != nil {
		log.Warning("Failed to get archived row count: %v", err)
	}
	log.Info("Archived %d events in %v", count, duration)

	return err
}

func (s *DataStore) EscalateAlertGroup(p core.AlertGroupQueryParams, user core.User) error {

	query := `UPDATE events SET escalated = 1 WHERE`

	builder := SqlBuilder{}

	builder.WhereEquals(
		"json_extract(events.source, '$.alert.signature_id')",
		p.SignatureID)

	builder.WhereEquals(
		"json_extract(events.source, '$.src_ip')",
		p.SrcIP)

	builder.WhereEquals(
		"json_extract(events.source, '$.dest_ip')",
		p.DstIP)

	if !p.MinTimestamp.IsZero() {
		builder.WhereGte("timestamp", p.MinTimestamp.UnixNano())
	}

	if !p.MaxTimestamp.IsZero() {
		builder.WhereLte("timestamp", p.MaxTimestamp.UnixNano())
	}

	query = strings.Replace(query, "WHERE", builder.BuildWhere(), 1)

	start := time.Now()

	log.Debug("Query: |%s|; Args: %v", query, builder.args)

	tx, err := s.db.GetTx()
	if err != nil {
		log.Error("%v", err)
	}
	defer tx.Commit()
	result, err := tx.Exec(query, builder.args...)
	if err != nil {
		log.Error("error starring alerts: %v", err)
		return err
	}
	count, _ := result.RowsAffected()

	duration := time.Now().Sub(start)
	log.Debug("Escalated/starred %d alerts in %v", count, duration)

	return err
}

func (s *DataStore) DeEscalateAlertGroup(p core.AlertGroupQueryParams, user core.User) error {

	query := `UPDATE events SET escalated = 0 WHERE`

	builder := SqlBuilder{}

	builder.WhereEquals(
		"json_extract(events.source, '$.alert.signature_id')",
		p.SignatureID)

	builder.WhereEquals(
		"json_extract(events.source, '$.src_ip')",
		p.SrcIP)

	builder.WhereEquals(
		"json_extract(events.source, '$.dest_ip')",
		p.DstIP)

	if !p.MinTimestamp.IsZero() {
		builder.WhereGte("timestamp", p.MinTimestamp.UnixNano())
	}

	if !p.MaxTimestamp.IsZero() {
		builder.WhereLte("timestamp", p.MaxTimestamp.UnixNano())
	}

	query = strings.Replace(query, "WHERE", builder.BuildWhere(), 1)

	tx, err := s.db.GetTx()
	if err != nil {
		log.Error("%v", err)
	}
	defer tx.Commit()
	r, err := tx.Exec(query, builder.args...)
	if err != nil {
		log.Error("error archiving alerts: %v", err)
		return err
	}
	count, err := r.RowsAffected()
	if err != nil {
		log.Error("Failed to de-escalate/unstarred events: %v", err)
		return err
	} else {
		log.Debug("De-escalated/unstarred %d events", count)
	}

	return err
}

func (s *DataStore) EventQuery(options core.EventQueryOptions) (interface{}, error) {

	size := int64(500)

	if options.Size > 0 {
		size = options.Size
	}

	query := `select events.rowid as id, events.archived, events.source`

	sqlBuilder := SqlBuilder{}

	sqlBuilder.From("events")

	sqlBuilder.Where("json_extract(events.source, '$.event_type') != 'stats'")

	if options.EventType != "" {
		sqlBuilder.WhereEquals("json_extract(events.source, '$.event_type')", options.EventType)
	}

	if options.QueryString != "" {
		parseQueryString(&sqlBuilder, options.QueryString, "events")
	}

	if !options.MaxTs.IsZero() {
		sqlBuilder.WhereLte("events.timestamp", options.MaxTs.UnixNano())
	}

	if !options.MinTs.IsZero() {
		sqlBuilder.WhereGte("events.timestamp", options.MinTs.UnixNano())
	}

	query += sqlBuilder.BuildFrom()

	if sqlBuilder.HasWhere() {
		query += sqlBuilder.BuildWhere()
	}

	if options.Order == "asc" {
		query += fmt.Sprintf(" ORDER BY events.timestamp ASC")
	} else {
		query += fmt.Sprintf(" ORDER BY events.timestamp DESC")
	}

	query += fmt.Sprintf(" LIMIT %d", size)

	tx, err := s.db.GetTx()
	if err != nil {
		return nil, err
	}
	defer tx.Commit()

	rows, err := tx.Query(query, sqlBuilder.args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	events := []interface{}{}

	for rows.Next() {
		var id int64
		var archived int8
		var rawSource []byte
		err = rows.Scan(&id, &archived, &rawSource)
		if err != nil {
			return nil, err
		}

		source, err := eve.NewEveEventFromBytes(rawSource)
		if err != nil {
			return nil, err
		}

		if archived > 0 {
			source.AddTag("evebox.archived")
			source.AddTag("archived")
		}

		source["@timestamp"] = source["timestamp"]

		events = append(events, map[string]interface{}{
			"_id":     id,
			"_source": source,
		})
	}

	return map[string]interface{}{
		"data": events,
	}, nil
}

func (d *DataStore) FindFlow(flowId uint64, proto string, timestamp string, srcIp string, destIp string) (interface{}, error) {

	query := `select
                    rowid as id, source
                  from events
                  where
                    json_extract(source, '$.event_type') = 'flow'
                    and json_extract(source, '$.flow_id') = ?
                    and (
                      (json_extract(source, '$.src_ip') = ?
                       and json_extract(source, '$.dest_ip') = ?)
                      or
                      (json_extract(source, '$.dest_ip') = ?
                       and json_extract(source, '$.src_ip') = ?)
                    )
                    and json_extract(source, '$.flow.start') <= ?
                    and json_extract(source, '$.flow.end') >= ?`

	timestamp, err := eveTs2SqliteTs(timestamp)
	if err != nil {
		log.Error("%v", err)
		return nil, err
	}

	tx, err := d.db.GetTx()
	if err != nil {
		log.Error("%v", err)
		return nil, err
	}
	defer tx.Commit()

	rows, err := tx.Query(query, flowId, srcIp, destIp, srcIp, destIp, timestamp, timestamp)
	if err != nil {
		log.Error("%v", err)
		return nil, err
	}
	defer rows.Close()

	events := []interface{}{}

	for rows.Next() {
		var id string
		var source []byte
		err := rows.Scan(&id, &source)
		if err != nil {
			log.Error("%v", err)
			return nil, err
		}
		event, err := eve.NewEveEventFromBytes(source)
		if err != nil {
			log.Error("%v", err)
			return nil, err
		}

		events = append(events, map[string]interface{}{
			"_id":     id,
			"_source": event,
		})
	}

	return events, nil
}

// Parse the query string and populate the sqlbuilder with parsed data.
//
// eventTable is the column name to join against events_fts.id, as it may
//   not always be the events table in case of aliasing.
func parseQueryString(builder *SqlBuilder, queryString string, eventTable string) {
	fts := []string{}

	parser := NewQueryStringParser(queryString)
	for {
		key, val := parser.Next()
		if key != "" && val != "" {

			var arg interface{}

			// Check if the value is a string...
			valInt, err := strconv.ParseInt(val, 0, 64)
			if err == nil {
				arg = valInt
			} else {
				arg = val
			}

			log.Debug("Parsed keyword: %s : %v", key, arg)

			switch {
			case key == "-tags" && arg == "archived":
				builder.WhereEquals("archived", 0)
			case (key == "tags" || key == "+tags") && arg == "archived":
				builder.WhereEquals("archived", 1)
			default:
				builder.WhereEquals(
					fmt.Sprintf(" json_extract(%s.source, '$.%s')", eventTable, key),
					arg)
			}

		} else if val != "" {
			fts = append(fts, fmt.Sprintf("\"%s\"", val))
		}
		if key == "" && val == "" {
			break
		}
	}

	if len(fts) > 0 {
		builder.From("events_fts")
		builder.Where(fmt.Sprintf("%s.rowid = events_fts.rowid", eventTable))
		builder.Where(fmt.Sprintf("events_fts MATCH '%s'", strings.Join(fts, " AND ")))
	}
}

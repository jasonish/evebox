// +build linux,amd64,cgo

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
	"bytes"
	"encoding/json"
	"fmt"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/elasticsearch"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"github.com/satori/go.uuid"
	"strconv"
	"strings"
	"time"
)

const DB_FILENAME = "evebox.sqlite"

type DataStore struct {
	core.NotImplementedEventService
	core.UnimplementedDatastore
	db *SqliteService
}

func NewDataStore(db *SqliteService) *DataStore {
	return &DataStore{
		db: db,
	}
}

func (d *DataStore) GetEveEventConsumer() core.EveEventConsumer {
	return NewSqliteIndexer(d.db)
}

func (s *DataStore) GetEventById(id string) (map[string]interface{}, error) {
	builder := SqlBuilder{}
	builder.Select("source")
	builder.From("events")
	builder.WhereEquals("id", id)

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

	return nil, core.NotImplementedError
}

func (s *DataStore) AlertQuery(options core.AlertQueryOptions) (interface{}, error) {

	query := `select
	            count(*) as count,
                    b.id as id,
                    sum(a.escalated),
                    b.archived,
                    min(a.timestamp) as mints,
                    max(a.timestamp) as maxts,
                    json_extract(a.source, '$.alert.signature') as signature,
                    json_extract(a.source, '$.src_ip') as src_ip,
                    json_extract(a.source, '$.dest_ip') as dest_ip,
                    b.source
                  %FROM%
                  join events b on
                    signature = json_extract(b.source, '$.alert.signature')
                    AND src_ip = json_extract(b.source, '$.src_ip')
                    AND dest_ip = json_extract(b.source, '$.dest_ip')
                  %WHERE%
                  group by
                    signature,
                    src_ip,
                    dest_ip
                  order by maxts desc`

	builder := SqlBuilder{}

	builder.From("events a")

	builder.Where("a.id = b.id")

	builder.WhereEquals("json_extract(a.source, '$.event_type')", "alert")

	if elasticsearch.StringSliceContains(options.MustHaveTags, "archived") {
		builder.WhereEquals("a.archived", 1)
	}

	if elasticsearch.StringSliceContains(options.MustNotHaveTags, "archived") {
		builder.WhereEquals("a.archived", 0)
	}

	if elasticsearch.StringSliceContains(options.MustHaveTags, "escalated") {
		builder.WhereEquals("b.escalated", 1)
	}

	if options.QueryString != "" {
		parseQueryString(&builder, options.QueryString, "a")
	}

	if options.TimeRange != "" {
		duration := parseTimeRange(options.TimeRange)
		if duration != "" {
			builder.Where(fmt.Sprintf(
				"a.timestamp >= strftime('%%Y-%%m-%%dT%%H:%%M:%%S.000000Z', 'now', '-%s seconds')", duration))
		}
	}

	query = strings.Replace(query, "%FROM%", builder.BuildFrom(), 1)
	query = strings.Replace(query, "%WHERE%", builder.BuildWhere(), 1)

	tx, err := s.db.GetTx()
	if err != nil {
		log.Error("%v", err)
		return nil, err
	}
	defer tx.Commit()
	rows, err := tx.Query(query, builder.args...)
	if err != nil {
		log.Error("%v", err)
		return nil, err
	}
	defer rows.Close()

	alerts := []interface{}{}

	for rows.Next() {

		var count int64
		var id string
		var escalated int64
		var archived int8
		var minTs string
		var maxTs string
		var signature string
		var srcIp string
		var destIp string
		var rawEvent []byte

		err = rows.Scan(&count,
			&id,
			&escalated,
			&archived,
			&minTs,
			&maxTs,
			&signature,
			&srcIp,
			&destIp,
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

		alert := map[string]interface{}{
			"count":          count,
			"escalatedCount": escalated,
			"minTs":          minTs,
			"maxTs":          maxTs,
			"event": map[string]interface{}{
				"_id":     id,
				"_source": event,
			},
		}

		alerts = append(alerts, alert)
	}

	return map[string]interface{}{
		"alerts": alerts,
	}, nil
}

func (s *DataStore) ArchiveAlertGroup(p core.AlertGroupQueryParams) error {

	b := SqlBuilder{}

	b.Select("id")
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
	if p.MinTimestamp != "" {
		ts, err := eveTs2SqliteTs(p.MinTimestamp)
		if err != nil {
			return err
		}
		b.WhereGte("timestamp", ts)
	}
	if p.MaxTimestamp != "" {
		ts, err := eveTs2SqliteTs(p.MaxTimestamp)
		if err != nil {
			return err
		}
		b.WhereLte("timestamp", ts)
	}

	query := fmt.Sprintf("UPDATE events SET archived = 1 WHERE id IN (%s)", b.Build())

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

func (s *DataStore) EscalateAlertGroup(p core.AlertGroupQueryParams) error {

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

	if p.MinTimestamp != "" {
		ts, err := eveTs2SqliteTs(p.MinTimestamp)
		if err != nil {
			return err
		}
		builder.WhereGte("timestamp", ts)
	}

	if p.MaxTimestamp != "" {
		ts, err := eveTs2SqliteTs(p.MaxTimestamp)
		if err != nil {
			return err
		}
		builder.WhereLte("timestamp", ts)
	}

	query = strings.Replace(query, "WHERE", builder.BuildWhere(), 1)

	start := time.Now()

	log.Println(query)

	tx, err := s.db.GetTx()
	if err != nil {
		log.Error("%v", err)
	}
	defer tx.Commit()
	result, err := tx.Exec(query, builder.args...)
	if err != nil {
		log.Error("error archiving alerts: %v", err)
		return err
	}
	count, _ := result.RowsAffected()

	duration := time.Now().Sub(start).Seconds()
	log.Info("Archived %d alerts, duration=%v", count, duration)

	return err
}

func (s *DataStore) UnstarAlertGroup(p core.AlertGroupQueryParams) error {

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

	if p.MinTimestamp != "" {
		ts, err := eveTs2SqliteTs(p.MinTimestamp)
		if err != nil {
			return err
		}
		builder.WhereGte("timestamp", ts)
	}

	if p.MaxTimestamp != "" {
		ts, err := eveTs2SqliteTs(p.MaxTimestamp)
		if err != nil {
			return err
		}
		builder.WhereLte("timestamp", ts)
	}

	query = strings.Replace(query, "WHERE", builder.BuildWhere(), 1)

	tx, err := s.db.GetTx()
	if err != nil {
		log.Error("%v", err)
	}
	defer tx.Commit()
	_, err = tx.Exec(query, builder.args...)
	if err != nil {
		log.Error("error archiving alerts: %v", err)
		return err
	}

	return err
}

func (s *DataStore) EventQuery(options core.EventQueryOptions) (interface{}, error) {

	size := int64(500)

	if options.Size > 0 {
		size = options.Size
	}

	query := `select events.id, events.timestamp, events.source`

	sqlBuilder := SqlBuilder{}

	sqlBuilder.From("events")

	sqlBuilder.Where("json_extract(events.source, '$.event_type') != 'stats'")

	if options.EventType != "" {
		sqlBuilder.WhereEquals("json_extract(events.source, '$.event_type')", options.EventType)
	}

	if options.QueryString != "" {
		parseQueryString(&sqlBuilder, options.QueryString, "events")
	}

	if options.MaxTs != "" {
		maxTs, err := time.Parse("2006-01-02T15:04:05.999999", options.MaxTs)
		if err != nil {
			return nil, fmt.Errorf("Bad timestamp: %s", options.MaxTs)
		}
		sqlBuilder.WhereLte("datetime(events.timestamp)", maxTs)
	}

	if options.MinTs != "" {
		minTs, err := time.Parse("2006-01-02T15:04:05.999999", options.MinTs)
		if err != nil {
			return nil, fmt.Errorf("Bad timestamp: %s", options.MinTs)
		}
		sqlBuilder.WhereGte("datetime(events.timestamp)", minTs)
	}

	query += sqlBuilder.BuildFrom()

	if sqlBuilder.HasWhere() {
		query += sqlBuilder.BuildWhere()
	}

	query += fmt.Sprintf(" ORDER BY events.timestamp DESC")
	query += fmt.Sprintf(" LIMIT %d", size)

	tx, err := s.db.GetTx()
	if err != nil {
		return nil, err
	}
	defer tx.Commit()

	log.Info("query: %s", query)

	rows, err := tx.Query(query, sqlBuilder.args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	events := []interface{}{}

	for rows.Next() {
		var rawSource []byte
		var id uuid.UUID
		var timestamp string
		err = rows.Scan(&id, &timestamp, &rawSource)
		if err != nil {
			return nil, err
		}

		source := map[string]interface{}{}

		decoder := json.NewDecoder(bytes.NewReader(rawSource))
		decoder.UseNumber()
		err = decoder.Decode(&source)
		if err != nil {
			return nil, err
		}

		source["@timestamp"] = timestamp

		events = append(events, map[string]interface{}{
			"_id":     id.String(),
			"_source": source,
		})
	}

	return map[string]interface{}{
		"data": events,
	}, nil
}

func (d *DataStore) FindFlow(flowId uint64, proto string, timestamp string, srcIp string, destIp string) (interface{}, error) {

	query := `select
                    id, source
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

// Parse the query string and populat the sqlbuilder with parsed data.
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

			builder.WhereEquals(
				fmt.Sprintf(" json_extract(%s.source, '$.%s')", eventTable, key),
				arg)
		} else if val != "" {
			fts = append(fts, fmt.Sprintf("\"%s\"", val))
		}
		if key == "" && val == "" {
			break
		}
	}

	if len(fts) > 0 {
		builder.From("events_fts")
		builder.Where(fmt.Sprintf("%s.id = events_fts.id", eventTable))
		builder.Where(fmt.Sprintf("events_fts MATCH '%s'", strings.Join(fts, " AND ")))
	}
}

func parseTimeRange(timeRange string) string {
	duration, err := time.ParseDuration(timeRange)
	if err != nil {
		log.Error("Failed to parse duration: %v", err)
		return ""
	}
	return strconv.FormatUint(uint64(duration.Seconds()), 10)
}

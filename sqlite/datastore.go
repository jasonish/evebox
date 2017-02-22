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
	"database/sql"
	"encoding/json"
	"fmt"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/elasticsearch"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"github.com/mattn/go-shellwords"
	"github.com/satori/go.uuid"
	"path"
	"strconv"
	"strings"
	"time"
)

const DB_FILENAME = "evebox.sqlite"

type DataStore struct {
	core.NotImplementedEventService
	db *SqliteService
}

func NewDataStore(dataDirectory string) (*DataStore, error) {
	db, err := NewSqliteService(path.Join(dataDirectory, DB_FILENAME))
	if err != nil {
		return nil, err
	}
	if err := db.Migrate(); err != nil {
		return nil, err
	}

	return &DataStore{
		db: db,
	}, nil
}

func (d *DataStore) GetEveEventConsumer() core.EveEventConsumer {
	return NewSqliteIndexer(d.db)
}

func (s *DataStore) AlertQuery(options core.AlertQueryOptions) (interface{}, error) {

	query := `select
	          count(json_extract(a.source, '$.alert.signature')),
	          case a.timestamp when max(a.timestamp) then a.id end,
	          b.source,
	          b.archived,
	          sum(b.escalated)
	         from events a
	         join events b on a.id = b.id
	         %WHERE%
	         group by
	           json_extract(a.source, '$.alert.signature'),
	           json_extract(a.source, '$.src_ip'),
	           json_extract(a.source, '$.dest_ip')
	         order by json_extract(a.source, '$.timestamp') DESC`

	builder := SqlBuilder{}

	builder.WhereEquals("json_extract(a.source, '$.event_type')", "alert")

	if elasticsearch.StringSliceContains(options.MustHaveTags, "archived") {
		builder.WhereEquals("b.archived", 1)
	}

	if elasticsearch.StringSliceContains(options.MustNotHaveTags, "archived") {
		builder.WhereEquals("b.archived", 0)
	}

	if elasticsearch.StringSliceContains(options.MustHaveTags, "escalated") {
		builder.WhereEquals("b.escalated", 1)
	}

	query = strings.Replace(query, "%WHERE%", builder.BuildWhere(), 1)

	var rows *sql.Rows
	var err error

	log.Println(query)

	tx, err := s.db.GetTx()
	if err != nil {
		log.Error("%v", err)
		return nil, err
	}
	defer tx.Commit()
	rows, err = tx.Query(query, builder.args...)
	if err != nil {
		return nil, err
	}
	defer rows.Close()

	alerts := []interface{}{}

	for rows.Next() {

		var count int64
		var id string
		var rawEvent []byte
		var archived int8
		var escalated int64

		err = rows.Scan(&count, &id, &rawEvent, &archived, &escalated)
		if err != nil {
			return nil, err
		}

		event, err := eve.NewEveEventFromBytes(rawEvent)
		if err != nil {
			return nil, err
		}

		log.Println("escalated: ", escalated)

		if archived > 0 {
			event["tags"] = append(event["tags"].([]interface{}),
				"archived")
		}

		alert := map[string]interface{}{
			"count":          count,
			"escalatedCount": escalated,
			"maxTs":          event["timestamp"],
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

	query := `UPDATE events SET archived = 1 WHERE`

	builder := SqlBuilder{}

	builder.WhereEquals("archived", 0)

	builder.WhereEquals(
		"json_extract(events.source, '$.alert.signature_id')",
		p.SignatureID)

	builder.WhereEquals(
		"json_extract(events.source, '$.src_ip')",
		p.SrcIP)

	builder.WhereEquals(
		"json_extract(events.source, '$.dest_ip')",
		p.DstIP)

	if p.MaxTimestamp != "" {
		ts, err := eveTs2SqliteTs(p.MaxTimestamp)
		if err != nil {
			return err
		}
		builder.WhereLte("timestamp", ts)
	}

	query = strings.Replace(query, "WHERE", builder.BuildWhere(), 1)

	start := time.Now()

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

func (s *DataStore) EscalateAlertGroup(p core.AlertGroupQueryParams) error {

	query := `UPDATE events SET escalated = 1 WHERE`

	builder := SqlBuilder{}

	builder.WhereEquals("archived", 0)

	builder.WhereEquals(
		"json_extract(events.source, '$.alert.signature_id')",
		p.SignatureID)

	builder.WhereEquals(
		"json_extract(events.source, '$.src_ip')",
		p.SrcIP)

	builder.WhereEquals(
		"json_extract(events.source, '$.dest_ip')",
		p.DstIP)

	if p.MaxTimestamp != "" {
		ts, err := eveTs2SqliteTs(p.MaxTimestamp)
		if err != nil {
			return err
		}
		builder.WhereLte("timestamp", ts)
	}

	query = strings.Replace(query, "WHERE", builder.BuildWhere(), 1)

	start := time.Now()

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

	builder.WhereEquals("archived", 0)

	builder.WhereEquals(
		"json_extract(events.source, '$.alert.signature_id')",
		p.SignatureID)

	builder.WhereEquals(
		"json_extract(events.source, '$.src_ip')",
		p.SrcIP)

	builder.WhereEquals(
		"json_extract(events.source, '$.dest_ip')",
		p.DstIP)

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

	if options.EventType != "" {
		sqlBuilder.WhereEquals("json_extract(events.source, '$.event_type')", options.EventType)
	}

	fts := []string{}

	if options.QueryString != "" {

		words, _ := shellwords.Parse(options.QueryString)

		for _, word := range words {

			log.Debug("Word: %s", word)

			parts := strings.SplitN(word, "=", 2)

			if len(parts) == 2 {

				field := parts[0]
				valuestr := parts[1]
				var arg interface{}

				valueint, err := strconv.ParseInt(valuestr, 0, 64)
				if err == nil {
					arg = valueint
				} else {
					arg = valuestr
				}

				sqlBuilder.WhereEquals(
					fmt.Sprintf(" json_extract(events.source, '$.%s')", field),
					arg)
			} else {
				fts = append(fts, fmt.Sprintf("\"%s\"", parts[0]))
			}

		}
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

	if len(fts) > 0 {
		sqlBuilder.From("events_fts")
		sqlBuilder.Where("events.id == events_fts.id")
		sqlBuilder.Where(fmt.Sprintf("events_fts MATCH '%s'", strings.Join(fts, " AND ")))
	}

	query += sqlBuilder.BuildFrom()

	if sqlBuilder.HasWhere() {
		query += sqlBuilder.BuildWhere()
	}

	query += fmt.Sprintf(" ORDER BY timestamp DESC")
	query += fmt.Sprintf(" LIMIT %d", size)

	log.Println(query)

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

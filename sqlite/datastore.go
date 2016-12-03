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
	"github.com/jasonish/evebox/log"
	"github.com/mattn/go-shellwords"
	"github.com/satori/go.uuid"
	"strconv"
	"strings"
	"time"
)

type DataStore struct {
	core.NotImplementedEventQueryService
	core.NotImplementedEventService
	db *SqliteService
}

func NewDataStore() (*DataStore, error) {
	db, err := NewSqliteService("evebox.db")
	if err != nil {
		return nil, err
	}
	return &DataStore{
		db: db,
	}, nil
}

func decodeRawEveEvent(rawBytes []byte) (map[string]interface{}, error) {
	decoder := json.NewDecoder(bytes.NewReader(rawBytes))
	decoder.UseNumber()
	var decoded map[string]interface{}
	err := decoder.Decode(&decoded)
	if err != nil {
		return nil, err
	}
	return decoded, nil
}

func (s *DataStore) AlertQuery(options core.AlertQueryOptions) (interface{}, error) {

	sql := `select
	          count(json_extract(a.source, '$.alert.signature')),
	          case a.timestamp when max(a.timestamp) then a.id end,
	          b.source,
	          b.archived
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

	sql = strings.Replace(sql, "%WHERE%", builder.BuildWhere(), 1)

	rows, err := s.db.Query(sql, builder.args...)
	if err != nil {
		return nil, err
	}

	alerts := []interface{}{}

	for rows.Next() {

		var count int64
		var id string
		var rawEvent []byte
		var archived int8

		err = rows.Scan(&count, &id, &rawEvent, &archived)
		if err != nil {
			return nil, err
		}

		event, err := decodeRawEveEvent(rawEvent)
		if err != nil {
			return nil, err
		}

		if archived > 0 {
			event["tags"] = append(event["tags"].([]interface{}),
				"archived")
		}

		alert := map[string]interface{}{
			"count":          count,
			"escalatedCount": 0,
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

	sql := `UPDATE events SET archived = 1 WHERE`

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

	sql = strings.Replace(sql, "WHERE", builder.BuildWhere(), 1)

	start := time.Now()
	r, err := s.db.DB.Exec(sql, builder.args...)
	if err != nil {
		return err
	}
	rows, _ := r.RowsAffected()
	log.Debug("Archived %d alerts.", rows)
	duration := time.Now().Sub(start).Seconds()
	log.Debug("Archive query time: %v", duration)

	return nil
}

func (s *DataStore) EventQuery(options core.EventQueryOptions) (interface{}, error) {

	size := int64(500)

	if options.Size > 0 {
		size = options.Size
	}

	sql := `select events.id, events.timestamp, events.source`

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

	sql += sqlBuilder.BuildFrom()

	if sqlBuilder.HasWhere() {
		sql += sqlBuilder.BuildWhere()
	}

	sql += fmt.Sprintf(" ORDER BY timestamp DESC")
	sql += fmt.Sprintf(" LIMIT %d", size)

	log.Println(sql)

	rows, err := s.db.Query(sql, sqlBuilder.args...)
	if err != nil {
		return nil, err
	}

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

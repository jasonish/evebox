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
	"encoding/json"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
)

type op struct {
	query string
	args  []interface{}
}

type SqliteIndexer struct {
	db    *SqliteService
	queue []op
}

func NewSqliteIndexer(db *SqliteService) *SqliteIndexer {
	return &SqliteIndexer{
		db: db,
	}
}

func (i *SqliteIndexer) Submit(event eve.EveEvent) error {

	// Convert flow timestamps for UTC.
	if event.GetString("event_type") == "flow" {
		startTs, err := eveTs2SqliteTs(
			event.GetMap("flow").GetString("start"))
		if err == nil {
			event.GetMap("flow")["start"] = startTs
		}
		endTs, err := eveTs2SqliteTs(
			event.GetMap("flow").GetString("end"))
		if err == nil {
			event.GetMap("flow")["end"] = endTs
		}
	}

	// Convert netflow timestamps to UTC.
	if event.GetString("event_type") == "netflow" {
		startTs, err := eveTs2SqliteTs(
			event.GetMap("netflow").GetString("start"))
		if err == nil {
			event.GetMap("netflow")["start"] = startTs
		}
		endTs, err := eveTs2SqliteTs(
			event.GetMap("netflow").GetString("end"))
		if err == nil {
			event.GetMap("netflow")["end"] = endTs
		}
	}

	encoded, err := json.Marshal(event)
	if err != nil {
		return err
	}

	i.queue = append(i.queue, op{
		query: "insert into events (timestamp, source) values ($1, $2)",
		args:  []interface{}{event.Timestamp().UnixNano(), encoded},
	})
	i.queue = append(i.queue, op{
		query: "insert into events_fts (rowid, source) values (last_insert_rowid(), $1)",
		args:  []interface{}{encoded},
	})

	return nil
}

func (i *SqliteIndexer) Commit() (interface{}, error) {
	queue := i.queue
	i.queue = nil

	tx, err := i.db.GetTx()
	if err != nil {
		log.Error("%v", err)
		return nil, err
	}

	for _, op := range queue {
		_, err := tx.Exec(op.query, op.args...)
		if err != nil {
			log.Error("%v", err)
			tx.Rollback()
			return nil, err
		}
	}

	return nil, tx.Commit()
}

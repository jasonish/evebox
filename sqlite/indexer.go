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
	"encoding/json"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"github.com/satori/go.uuid"
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

	encoded, err := json.Marshal(event)
	if err != nil {
		return err
	}

	eventId := uuid.NewV1()

	timestamp, err := eveTs2SqliteTs(event["timestamp"].(string))
	if err != nil {
		return err
	}

	i.queue = append(i.queue, op{
		query: "insert into events values ($1, $2, 0, 0, $3)",
		args:  []interface{}{eventId, timestamp, encoded},
	})
	i.queue = append(i.queue, op{
		query: "insert into events_fts values ($1, $2, $3)",
		args:  []interface{}{eventId, timestamp, encoded},
	})

	return nil
}

func (i *SqliteIndexer) Commit() (interface{}, error) {
	queue := i.queue
	i.queue = nil

	tx, err := i.db.Begin()
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

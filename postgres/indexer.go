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
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"github.com/satori/go.uuid"
)

type PgEventIndexer struct {
	pg     *PgDB
	tables map[string]bool
	tx     *sql.Tx
}

func NewPgEventIndexer(pg *PgDB) *PgEventIndexer {
	indexer := &PgEventIndexer{}
	indexer.pg = pg
	indexer.tables = make(map[string]bool)
	return indexer
}

func (i *PgEventIndexer) CreateTable(timestamp string) {
	tx, err := i.pg.Begin()
	if err != nil {
		log.Warning("Failed to begin transaction to create event table %s", timestamp)
		return
	}

	_, err = tx.Exec("select evebox_create_events_table($1)", timestamp)
	if err != nil {
		log.Warning("Failed to create event table %s: %v", timestamp, err)
		tx.Rollback()
		return
	}
	err = tx.Commit()
	if err != nil {
		log.Warning("Failed to commit create table %s.", timestamp)
	}
	i.tables[timestamp] = true
}

func (i *PgEventIndexer) Submit(event eve.EveEvent) error {

	timestamp := event.Timestamp()
	yyyymmdd := timestamp.UTC().Format("20060102")

	if !i.tables[yyyymmdd] {
		i.CreateTable(yyyymmdd)
	}

	if i.tx == nil {
		var err error
		i.tx, err = i.pg.Begin()
		if err != nil {
			return err
		}
	}

	encoded, err := json.Marshal(event)
	if err != nil {
		log.Error("Failed to marshal event to JSON: %v", err)
		return err
	}

	id := uuid.NewV1()

	var archived bool

	if event.EventType() == "alert" {
		archived = false
	} else {
		archived = true
	}

	eventsSql := fmt.Sprintf(`insert into events_%s
	    (uuid, timestamp, archived)
	    values ($1, $2, $3)`,
		yyyymmdd)

	_, err = i.tx.Exec(eventsSql,
		id,
		timestamp,
		archived)
	if err != nil {
		log.Fatal(err)
	}

	sourceSql := fmt.Sprintf(`insert into events_source_%s
	    (uuid, timestamp, source)
	    values ($1, $2, $3)`,
		yyyymmdd)

	_, err = i.tx.Exec(sourceSql,
		id,
		timestamp,
		encoded)
	if err != nil {
		log.Fatal(err)
	}

	return nil
}

func (i *PgEventIndexer) Commit() (interface{}, error) {
	err := i.tx.Commit()
	i.tx = nil
	return nil, err
}

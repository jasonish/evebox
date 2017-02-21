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
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"github.com/satori/go.uuid"
)

type Indexer struct {
	db   *sql.DB
	tx   *sql.Tx
	stmt *sql.Stmt
}

func NewIndexer(service *Service) (*Indexer, error) {
	tx, err := service.db.Begin()
	if err != nil {
		return nil, err
	}

	stmt, err := tx.Prepare("insert into events_master (uuid, timestamp, source) values ($1, $2, $3)")
	if err != nil {
		log.Fatal(err)
	}

	return &Indexer{
		db:   service.db,
		tx:   tx,
		stmt: stmt,
	}, nil
}

func (i *Indexer) AddEvent(event eve.EveEvent) error {
	uuid := uuid.NewV1()
	timestamp := event.Timestamp()
	encoded, err := json.Marshal(&event)
	if err != nil {
		log.Error("Failed to encode event.")
	}

	_, err = i.stmt.Exec(uuid, timestamp, string(encoded))
	if err != nil {
		log.Fatal(err)
	}

	return nil
}

func (i *Indexer) Flush() error {

	err := i.tx.Commit()
	if err != nil {
		return err
	}
	i.tx, err = i.db.Begin()
	if err != nil {
		return err
	}

	i.stmt, err = i.tx.Prepare("insert into events_master (uuid, timestamp, source) values ($1, $2, $3)")
	if err != nil {
		log.Fatal(err)
	}

	return nil
}

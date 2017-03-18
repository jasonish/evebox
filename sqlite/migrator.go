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
	"database/sql"
	"fmt"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/resources"
	"os"
)

type Migrator struct {
	db *SqliteService
}

func NewMigrator(db *SqliteService) *Migrator {
	return &Migrator{
		db: db,
	}
}

func (m *Migrator) Migrate() error {

	var currentVersion int
	nextVersion := 0

	rows, err := m.db.Query("select max(version) from schema")
	if err == nil {
		if rows.Next() {
			if err := rows.Scan(&currentVersion); err != nil {
				return err
			}
			nextVersion = currentVersion + 1
		}
		log.Debug("Current database schema version: %d", currentVersion)
	} else {
		log.Debug("Initializing database.")
	}

	for {

		script, err := resources.AssetString(fmt.Sprintf("sqlite/V%d.sql", nextVersion))
		if err != nil {
			break
		}

		log.Info("Updating database to version %d.", nextVersion)

		tx, err := m.db.Begin()
		if err != nil {
			return err
		}

		_, err = tx.Exec(script)
		if err != nil {
			return err
		}

		err = m.setVersion(tx, nextVersion)
		if err != nil {
			return err
		}

		err = tx.Commit()
		if err != nil {
			return err
		}

		nextVersion++
	}

	return nil
}

func (m *Migrator) setVersion(tx *sql.Tx, version int) error {
	_, err := tx.Exec(`insert into schema (version, timestamp)
	                     values ($1, datetime('now'))`, version)
	return err
}

func (m *Migrator) fileExists(path string) bool {
	_, err := os.Stat(path)
	if err == nil {
		return true
	}
	return false
}

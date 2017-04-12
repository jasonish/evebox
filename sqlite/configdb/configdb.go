/* Copyright (c) 2017 Jason Ish
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

package configdb

import (
	"database/sql"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/sqlite/sqlcommon"
	_ "github.com/mattn/go-sqlite3"
	"os"
	"path"
)

const driver = "sqlite3"
const filename = "config.sqlite"

type ConfigDB struct {
	DB       *sql.DB
	InMemory bool
}

func NewConfigDB(directory string) (*ConfigDB, error) {

	var dsn string
	var inMemory bool

	if directory == ":memory:" {
		log.Info("Using in-memory configuration DB.")
		dsn = ":memory:"
		inMemory = true
	} else {
		dsn = path.Join(directory, filename)
		_, err := os.Stat(dsn)
		if err == nil {
			log.Info("Using configuration database file %s", dsn)
		} else {
			log.Info("Creating new configuration database %s", dsn)
		}
	}

	db, err := sql.Open(driver, dsn)
	if err != nil {
		return nil, err
	}
	configDB := &ConfigDB{
		DB:       db,
		InMemory: inMemory,
	}

	if err := configDB.migrate(); err != nil {
		return nil, err
	}

	return configDB, nil
}

func (db *ConfigDB) migrate() error {
	migrator := sqlcommon.NewSqlMigrator(db.DB, "configdb")
	return migrator.Migrate()
}

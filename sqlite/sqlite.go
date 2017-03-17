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
	"github.com/jasonish/evebox/server"
	_ "github.com/mattn/go-sqlite3"
	"github.com/spf13/viper"
	"path"
	"time"
)

type SqliteService struct {
	*sql.DB
}

func NewSqliteService(filename string) (*SqliteService, error) {

	log.Debug("Opening SQLite database %s", filename)
	dsn := fmt.Sprintf("file:%s?cache=shared&mode=rwc&_txlock=immediate",
		filename)
	db, err := sql.Open("sqlite3", dsn)
	if err != nil {
		return nil, err
	}

	service := &SqliteService{
		DB: db,
	}

	return service, nil
}

func (s *SqliteService) GetTx() (tx *sql.Tx, err error) {
	for i := 0; i < 100; i++ {
		tx, err = s.DB.Begin()
		if err == nil {
			return tx, nil
		} else {
			time.Sleep(10 * time.Millisecond)
		}
	}
	return nil, err
}

func (s *SqliteService) Migrate() error {
	migrator := NewMigrator(s)
	return migrator.Migrate()
}

func InitPurger(db *SqliteService) {
	retentionPeriod := viper.GetInt("database.retention-period")
	log.Info("Retention period: %d days", retentionPeriod)

	// Start the purge runner.
	go (&SqlitePurger{
		db:     db,
		period: retentionPeriod,
	}).Run()
}

func InitSqlite(appContext *server.AppContext) (err error) {

	log.Info("Configuring SQLite datastore")

	filename := viper.GetString("database.sqlite.filename")
	if filename == "" {
		filename = DB_FILENAME
	}
	if filename != ":memory:" {
		directory := viper.GetString("data-directory")
		if directory == "." {
			log.Warning("Using current directory as the data directory, you may want to set the data-directory option")
		}
		filename = path.Join(viper.GetString("data-directory"), filename)
	}

	db, err := NewSqliteService(filename)
	if err != nil {
		return err
	}

	if err := db.Migrate(); err != nil {
		return err
	}

	appContext.DataStore = NewDataStore(db)

	InitPurger(db)

	return nil
}

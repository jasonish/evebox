// +build linux,amd64,cgo

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

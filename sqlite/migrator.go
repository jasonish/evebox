package sqlite

import (
	"database/sql"
	"fmt"
	"github.com/jasonish/evebox/log"
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

	}

	log.Debug("Current database schema version: %d", currentVersion)

	for {

		path := fmt.Sprintf("resources/sqlite/V%d.sql", nextVersion)
		if !m.fileExists(path) {
			break
		}

		log.Info("Updating database with %s.", path)

		tx, err := m.db.Begin()
		if err != nil {
			return err
		}

		err = m.db.TxLoadScript(tx, path)
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

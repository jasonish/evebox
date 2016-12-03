// +build linux,amd64,cgo

package sqlite

import (
	"database/sql"
	"encoding/json"
	"github.com/jasonish/evebox/eve"
	"github.com/satori/go.uuid"
)

type SqliteIndexer struct {
	db *SqliteService
	tx *sql.Tx
}

func NewSqliteIndexer(db *SqliteService) (*SqliteIndexer, error) {

	tx, err := db.Begin()
	if err != nil {
		return nil, err
	}

	return &SqliteIndexer{
		db: db,
		tx: tx,
	}, nil
}

func (i *SqliteIndexer) IndexRawEve(event eve.RawEveEvent) error {

	encoded, err := json.Marshal(event)
	if err != nil {
		return err
	}

	eventId := uuid.NewV1()

	timestamp, err := eveTs2SqliteTs(event["timestamp"].(string))
	if err != nil {
		return err
	}

	_, err = i.tx.Exec("insert into events values ($1, $2, 0, 0, $3)", eventId,
		timestamp, encoded)
	if err != nil {
		return err
	}

	_, err = i.tx.Exec("insert into events_fts values ($1, $2)", eventId,
		encoded)
	if err != nil {
		return err
	}

	return nil
}

func (i *SqliteIndexer) Flush() (err error) {

	err = i.tx.Commit()
	if err != nil {
		return err
	}

	i.tx, err = i.db.Begin()
	if err != nil {
		return err
	}

	return nil
}

package sqlite

import (
	"database/sql"
	"encoding/json"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"github.com/satori/go.uuid"
	"time"
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

	timestamp, err := FormatTimestamp(event["timestamp"].(string))
	if err != nil {
		return err
	}

	_, err = i.tx.Exec("insert into events values ($1, $2, 0, 0, $3)", eventId,
		timestamp, encoded)
	if err != nil {
		log.Fatal(err)
	}

	_, err = i.tx.Exec("insert into events_fts values ($1, $2)", eventId,
		encoded)
	if err != nil {
		log.Fatal(err)
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

// Format an event timestamp for use in a SQLite column. The format is
// already correct, just needs to be converted to UTC.
func FormatTimestamp(timestamp string) (string, error) {
	var RFC3339Nano_Modified string = "2006-01-02T15:04:05.999999999Z0700"
	result, err := time.Parse(RFC3339Nano_Modified, timestamp)
	if err != nil {
		return "", err
	}
	return result.UTC().Format("2006-01-02T15:04:05.999999999"), nil
}

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

func (i *Indexer) AddEvent(event eve.RawEveEvent) error {
	uuid := uuid.NewV1()
	timestamp, err := event.GetTimestamp()
	if err != nil {
		log.Error("Failed to get timestamp from event: %v", err)
	}
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

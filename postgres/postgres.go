package postgres

import (
	_ "github.com/lib/pq"
	"database/sql"
	"fmt"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/eve"
	"github.com/satori/go.uuid"
	"encoding/json"
)

const PGDATABASE = "evebox"
const PGUSER = "evebox"
const PGPASS = "evebox"
const PGPORT = "8432"

type Service struct {
	db *sql.DB
}

func NewService() *Service {
	args := fmt.Sprintf(
		"dbname=%s user=%s password=%s port=%s sslmode=%s",
		PGDATABASE,
		PGUSER,
		PGPASS,
		PGPORT,
		"disable")
	db, err := sql.Open("postgres", args)
	if err != nil {
		log.Fatal(err)
	}
	log.Println(db)

	// Do something, so we know we are really connected.
	//var schemaVersion int64
	//err = db.QueryRow("select max(version) from schema").Scan(&schemaVersion)
	//if err != nil {
	//	log.Fatal(err)
	//}

	return &Service{
		db: db,
	}
}

func (s *Service) AddEvent(event eve.RawEveEvent) {

	uuid := uuid.NewV1()
	timestamp, err := event.GetTimestamp()
	if err != nil {
		log.Error("Failed to get timestamp from event: %v", err)
	}
	encoded, err := json.Marshal(&event)
	if err != nil {
		log.Error("Failed to encode event.")
	}
	_, _ = s.db.Exec(`
	    insert into events_master (uuid, timestamp, source)
	    values ($1, $2, $3)`, uuid, timestamp, string(encoded))
}

type Indexer struct {
	db *sql.DB
	tx *sql.Tx
}

func NewIndexer(service *Service) (*Indexer, error) {
	tx, err := service.db.Begin()
	if err != nil {
		return nil, err
	}
	return &Indexer{
		db: service.db,
		tx: tx,
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
	_, _ = i.tx.Exec(`
	    insert into events_master (uuid, timestamp, source)
	    values ($1, $2, $3)`, uuid, timestamp, string(encoded))
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
	return nil
}
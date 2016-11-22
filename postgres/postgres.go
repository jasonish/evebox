package postgres

import (
	"database/sql"
	"encoding/json"
	"fmt"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	_ "github.com/lib/pq"
	"github.com/satori/go.uuid"
)

const PGDATABASE = "evebox"
const PGUSER = "evebox"
const PGPASS = "evebox"
const PGPORT = "8432"

type Service struct {
	db *sql.DB
}

func NewService() (*Service, error) {
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

	var pgVersion string

	err = db.QueryRow("select version()").Scan(&pgVersion)
	if err != nil {
		return nil, err
	}
	log.Info("Connected to PostgreSQL version %s.", pgVersion)

	return &Service{
		db: db,
	}, nil
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

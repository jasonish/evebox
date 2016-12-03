package postgres

import (
	"database/sql"
	"fmt"
	"github.com/jasonish/evebox/log"
	_ "github.com/lib/pq"
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

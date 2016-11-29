package sqlite

import "database/sql"
import (
	_ "github.com/mattn/go-sqlite3"
	"io/ioutil"
	"os"
)

type SqliteService struct {
	*sql.DB
}

func NewSqliteService(filename string) (*SqliteService, error) {

	db, err := sql.Open("sqlite3", filename)
	if err != nil {
		return nil, err
	}

	return &SqliteService{
		db,
	}, nil
}

func (s *SqliteService) Migrate() error {
	migrator := NewMigrator(s)
	return migrator.Migrate()
}

func (s *SqliteService) LoadScript(filename string) error {
	file, err := os.Open(filename)
	if err != nil {
		return err
	}
	buf, err := ioutil.ReadAll(file)
	if err != nil {
		return err
	}
	_, err = s.Exec(string(buf))
	return err
}

func (s *SqliteService) TxLoadScript(tx *sql.Tx, filename string) error {
	file, err := os.Open(filename)
	if err != nil {
		return err
	}
	buf, err := ioutil.ReadAll(file)
	if err != nil {
		return err
	}
	_, err = tx.Exec(string(buf))
	return err
}

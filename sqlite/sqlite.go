package sqlite

import "database/sql"
import (
	_ "github.com/mattn/go-sqlite3"
	"os"
	"io/ioutil"
)

type SqliteService struct {
	*sql.DB
}

func NewSqliteService() (*SqliteService, error) {

	db, err := sql.Open("sqlite3", "sqlite.db")
	if err != nil {
		return nil, err
	}

	return &SqliteService{
		db,
	}, nil
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
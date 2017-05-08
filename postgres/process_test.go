package postgres

import (
	"github.com/jasonish/evebox/log"
	_ "github.com/lib/pq"
	"github.com/stretchr/testify/require"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func TestPostgresGetVersion(t *testing.T) {
	version, err := GetVersion()
	if err != nil {
		t.Fatal(err)
	}
	log.Println(version)
}

func TestPostgresInitStartStop(t *testing.T) {
	r := require.New(t)

	directory, err := filepath.Abs("TestPostgresStartStop.pgdata")
	r.Nil(err)
	defer os.RemoveAll(directory)

	manager, err := NewPostgresManager(directory)
	r.Nil(err)
	r.NotNil(manager)
	r.False(manager.IsInitialized())

	err = manager.Init()
	r.Nil(err)
	r.True(manager.IsInitialized())

	err = manager.Start()
	r.Nil(err)
	defer manager.StopFast()

	// Attempt to connect.
	pgConfig, err := ManagedConfig(directory)
	r.Nil(err)

	db, err := NewPgDatabase(pgConfig)
	r.Nil(err)
	defer db.Close()

	rows, err := db.Query("SELECT version()")
	r.Nil(err)
	defer rows.Close()
	r.True(rows.Next())
	var version string
	r.Nil(rows.Scan(&version))
	r.Equal(0, strings.Index(version, "PostgreSQL"))

	migrator := NewSqlMigrator(db, "postgres")
	err = migrator.Migrate()
	r.Nil(err)
}

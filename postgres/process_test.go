package postgres

import (
	"testing"
	"log"
	"os"
	"time"
)

func TestPostgresGetVersion(t *testing.T) {
	version, err := GetVersion()
	if err != nil {
		t.Fatal(err)
	}
	log.Println(version)
}

func TestPostgresInitStartStop(t *testing.T) {
	datadir := "TestPostgresStartStop.pgdata"

	os.RemoveAll(datadir)
	err := Init(datadir)
	defer os.RemoveAll(datadir)
	if err != nil {
		t.Fatal(err)
	}

	command, err := Start(datadir)

	time.Sleep(100 * time.Millisecond)

	// Kill...
	Stop(command)

	err = command.Wait()
	if err != nil {
		t.Fatal(err)
	}
}

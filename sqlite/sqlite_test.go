package sqlite

import (
	"testing"
	"database/sql"
	"github.com/jasonish/evebox/log"

	_ "github.com/mattn/go-sqlite3"
	"os"
	"io/ioutil"
	"github.com/jasonish/evebox/evereader"
	"github.com/jasonish/evebox/elasticsearch"
	"github.com/satori/go.uuid"
	"time"
	"github.com/jasonish/evebox/eve"
)

func TestSqlite(t *testing.T) {

	os.Remove("./sqlite.db")

	db, err := sql.Open("sqlite3", "./sqlite.db")
	if err != nil {
		log.Fatal(err)
	}
	log.Println(db)

	v0File, err := os.Open("../resources/sqlite/V0.sql")
	if err != nil {
		log.Fatal(err)
	}
	v0, err := ioutil.ReadAll(v0File)
	if err != nil {
		log.Fatal(err)
	}

	res, err := db.Exec(string(v0))
	if err != nil {
		t.Fatal(err)
	}
	log.Println(res)

	reader, err := evereader.New("/var/log/suricata/eve.json")
	if err != nil {
		t.Fatal(err)
	}

	count := uint64(0)
	lastStatTs := time.Now()
	lastStatCount := uint64(0)

	tx, err := db.Begin()
	if err != nil {
		log.Fatal(err)
	}

	lastCommit := time.Now()

	tagsFilter := eve.TagsFilter{}

	for {
		next, err := reader.Next()
		if err != nil {
			t.Fatal(err)
		}

		tagsFilter.Filter(next)

		id := uuid.NewV4()

		timestamp := next["timestamp"].(string)
		timestamp = FormatTimestamp(timestamp)

		_, err = tx.Exec("insert into events values ($1, $2, $3)", id, timestamp, elasticsearch.ToJson(next))
		if err != nil {
			log.Fatal(err)
		}

		_, err = tx.Exec("insert into events_fts values ($1, $2)", id, elasticsearch.ToJson(next))
		if err != nil {
			log.Fatal(err)
		}

		count++


		now := time.Now()

		if now.Sub(lastCommit).Nanoseconds() > 100000000 {
			lastCommit = now
			tx.Commit()
			tx, err = db.Begin()
			if err != nil {
				log.Fatal(err)
			}
		}

		if now.Sub(lastStatTs).Seconds() > 1 {
			log.Info("Total: %d; Last minute: %d; Avg: %.2f/s",
				count,
				count-lastStatCount,
				float64(count-lastStatCount)/(now.Sub(lastStatTs).Seconds()))
			lastStatTs = now
			lastStatCount = count
		}

	}
}

// Format an event timestamp for use in a SQLite column. The format is
// already correct, just needs to be converted to UTC.
func FormatTimestamp(timestamp string) string {
	var RFC3339Nano_Modified string = "2006-01-02T15:04:05.999999999Z0700"
	result, err := time.Parse(RFC3339Nano_Modified, timestamp)
	if err != nil {
		log.Fatal(err)
	}
	return result.UTC().Format("2006-01-02T15:04:05.999999999")
}
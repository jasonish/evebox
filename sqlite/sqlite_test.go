package sqlite

import (
	"testing"
	"github.com/jasonish/evebox/log"

	"github.com/jasonish/evebox/evereader"
	"time"
	"github.com/jasonish/evebox/eve"
	"os"
)

func TestSqlite(t *testing.T) {

	os.Remove("./sqlite.db")

	db, err := NewSqliteService()
	if err != nil {
		log.Fatal()
	}

	err = db.LoadScript("../resources/sqlite/V0.sql")
	if err != nil {
		log.Fatal(err)
	}

	reader, err := evereader.New("/var/log/suricata/eve.json")
	if err != nil {
		t.Fatal(err)
	}

	count := uint64(0)
	lastStatTs := time.Now()
	lastStatCount := uint64(0)

	indexer, err := NewSqliteIndexer(db)
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

		indexer.IndexRawEve(next)

		count++

		now := time.Now()

		if now.Sub(lastCommit).Nanoseconds() > 100000000 {
			err = indexer.Flush()
			if err != nil {
				log.Fatal(err)
			}
			lastCommit = now
		}

		if now.Sub(lastStatTs).Seconds() > 1 {
			log.Info("Total: %d; Last minute: %d; Avg: %.2f/s",
				count,
				count - lastStatCount,
				float64(count - lastStatCount) / (now.Sub(lastStatTs).Seconds()))
			lastStatTs = now
			lastStatCount = count
		}

	}
}

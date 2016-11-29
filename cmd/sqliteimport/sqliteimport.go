// +build cgo

package sqliteimport

import (
	"fmt"
	"github.com/jasonish/evebox/evereader"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/sqlite"
	flag "github.com/spf13/pflag"
	"io"
	"time"
)

func Main(args []string) {

	var end bool
	var oneshot bool
	var useBookmark bool
	var bookmarkPath string
	var verbose bool
	var dbfile string

	flagset := flag.NewFlagSet("sqliteimport", flag.ExitOnError)
	flagset.StringVarP(&dbfile, "database", "D", "", "Database filename")
	flagset.BoolVar(&end, "end", false, "Start at end of file")
	flagset.BoolVar(&oneshot, "oneshot", false, "One shot mode (exit on EOF)")
	flagset.BoolVar(&useBookmark, "bookmark", false, "Bookmark location")
	flagset.StringVar(&bookmarkPath, "bookmark-path", "", "Path to bookmark file")
	flagset.BoolVarP(&verbose, "verbose", "v", false, "Verbose output")
	flagset.Parse(args)

	if verbose {
		log.SetLevel(log.DEBUG)
	}

	if dbfile == "" {
		log.Fatal("Database filename must be provided.")
	}

	if len(flagset.Args()) == 0 {
		log.Fatal("No input files provided.")
	} else if len(flagset.Args()) > 1 {
		log.Fatal("Only one input file allowed.")
	}
	inputFilename := flagset.Args()[0]

	// If useBookmark but no path, set a default.
	if useBookmark && bookmarkPath == "" {
		bookmarkPath = fmt.Sprintf("%s.bookmark", inputFilename)
	}

	reader, err := evereader.New(flagset.Args()[0])
	if err != nil {
		log.Fatal(err)
	}

	// Initialize bookmark.
	var bookmarker *evereader.Bookmarker = nil
	if useBookmark {
		bookmarker = &evereader.Bookmarker{
			Filename: bookmarkPath,
			Reader:   reader,
		}
		err := bookmarker.Init(end)
		if err != nil {
			log.Fatal(err)
		}
	} else if end {
		log.Info("Jumping to end of file.")
		err := reader.SkipToEnd()
		if err != nil {
			log.Fatal(err)
		}
	}

	db, err := sqlite.NewSqliteService(dbfile)
	if err != nil {
		log.Fatal(err)
	}
	if err := db.Migrate(); err != nil {
		log.Fatal(err)
	}

	indexer, err := sqlite.NewSqliteIndexer(db)
	if err != nil {
		log.Fatal(err)
	}

	count := uint64(0)
	lastStatTs := time.Now()
	lastStatCount := uint64(0)

	// Number of EOFs in last stat interval.
	eofs := uint64(0)

	for {
		eof := false
		event, err := reader.Next()
		if err != nil {
			if err == io.EOF {
				eof = true
				eofs++
			} else {
				log.Fatal(err)
			}
		}

		if event != nil {
			indexer.IndexRawEve(event)
			count++
			if useBookmark {
				bookmark := bookmarker.GetBookmark()
				bookmarker.WriteBookmark(bookmark)
			}
		}

		now := time.Now()

		if now.Sub(lastStatTs).Seconds() > 1 {
			log.Info("Total: %d; Last interval: %d; Avg: %.2f/s, EOFs: %d",
				count,
				count-lastStatCount,
				float64(count-lastStatCount)/(now.Sub(lastStatTs).Seconds()),
				eofs)
			lastStatTs = now
			lastStatCount = count
			eofs = 0
			indexer.Flush()
		}

		if eof {
			if oneshot {
				break
			} else {
				indexer.Flush()
				time.Sleep(100 * time.Millisecond)
			}
		}

	}

	now := time.Now()

	log.Info("Total: %d; Last interval: %d; Avg: %.2f/s, EOFs: %d",
		count,
		count-lastStatCount,
		float64(count-lastStatCount)/(now.Sub(lastStatTs).Seconds()),
		eofs)

	indexer.Flush()
}

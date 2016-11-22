package pgimport

import (
	"fmt"
	"github.com/jasonish/evebox/evereader"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/postgres"
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

	flagset := flag.NewFlagSet("evereader", flag.ExitOnError)
	flagset.BoolVar(&end, "end", false, "Start at end of file")
	flagset.BoolVar(&oneshot, "oneshot", false, "One shot mode (exit on EOF)")
	flagset.BoolVar(&useBookmark, "bookmark", false, "Bookmark location")
	flagset.StringVar(&bookmarkPath, "bookmark-path", "", "Path to bookmark file")
	flagset.BoolVarP(&verbose, "verbose", "v", false, "Verbose output")
	flagset.Parse(args)

	if verbose {
		log.SetLevel(log.DEBUG)
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

	pg := postgres.NewService()
	indexer, err := postgres.NewIndexer(pg)
	if err != nil {
		log.Fatal(err)
	}

	for {
		eof := false
		event, err := reader.Next()
		if err != nil {
			if err == io.EOF {
				if oneshot {
					break
				}
				eof = true
			} else {
				log.Fatal(err)
			}
		}

		if eof {
			time.Sleep(1 * time.Second)
		} else {

			indexer.AddEvent(event)

			if useBookmark {
				bookmark := bookmarker.GetBookmark()
				bookmarker.WriteBookmark(bookmark)
			}
		}
	}

	indexer.Flush()
}

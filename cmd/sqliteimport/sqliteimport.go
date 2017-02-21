// +build linux,amd64,cgo

/* Copyright (c) 2016 Jason Ish
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 *
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED ``AS IS'' AND ANY EXPRESS OR IMPLIED
 * WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * DISCLAIMED. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY DIRECT,
 * INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
 * (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING
 * IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
 * POSSIBILITY OF SUCH DAMAGE.
 */

package sqliteimport

import (
	"fmt"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/evereader"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/sqlite"
	"github.com/spf13/pflag"
	"io"
	"time"
)

func MainLoop(reader *evereader.EveReader, indexer core.EveEventConsumer,
	bookmarker *evereader.Bookmarker, oneshot bool) {

	eofs := 0
	count := 0
	lastStatTs := time.Now()
	lastStatCount := 0
	lastCommitTs := time.Now()

	tagsFilter := eve.TagsFilter{}

	logInterval := 60

	if log.GetLevel() == log.DEBUG {
		logInterval = 1
	}

	for {
		eof := false

		event, err := reader.Next()
		if err != nil {
			if err == io.EOF {
				eof = true
				eofs++
			}
		}

		if event != nil {
			tagsFilter.Filter(event)
			indexer.Submit(event)
			count++
		}

		now := time.Now()

		// Commit every 100ms or so...
		if eof || now.Sub(lastCommitTs).Seconds() >= 0.1 {
			indexer.Commit()
			lastCommitTs = time.Now()

			if bookmarker != nil {
				bookmark := bookmarker.GetBookmark()
				err := bookmarker.WriteBookmark(bookmark)
				if err != nil {
					log.Error("Failed to update bookmark: %v", err)
				}
			}
		}

		if now.Sub(lastStatTs).Seconds() >= float64(logInterval) {
			log.Info("Total: %d; Last interval: %d; Avg: %.2f/s, EOFs: %d",
				count,
				count-lastStatCount,
				float64(count-lastStatCount)/(now.Sub(lastStatTs).Seconds()),
				eofs)
			lastStatTs = now
			lastStatCount = count
			eofs = 0
		}

		if eof {
			if oneshot {
				indexer.Commit()
				break
			} else {
				indexer.Commit()
				time.Sleep(100 * time.Millisecond)
			}
		}
	}

}

func Main(args []string) {

	var end bool
	var oneshot bool
	var useBookmark bool
	var bookmarkPath string
	var verbose bool
	var dbfile string

	flagset := pflag.NewFlagSet("sqliteimport", pflag.ExitOnError)
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

	indexer := sqlite.NewSqliteIndexer(db)
	MainLoop(reader, indexer, bookmarker, oneshot)
	db.Close()
}

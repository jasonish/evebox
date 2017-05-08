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

package pgimport

import (
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/evereader"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/postgres"
	flag "github.com/spf13/pflag"
	"io"
	"runtime"
	"sync"
	"time"
)

func Main(args []string) {

	var verbose bool
	var dataDirectory string
	useNow := false

	flagset := flag.NewFlagSet("pgimport", flag.ExitOnError)
	flagset.BoolVarP(&verbose, "verbose", "v", false, "Verbose output")
	flagset.StringVarP(&dataDirectory, "data-directory", "D", "", "Data directory")
	flagset.BoolVar(&useNow, "now", false, "Use current time")
	flagset.Parse(args)

	if verbose {
		log.SetLevel(log.DEBUG)
	}

	if dataDirectory == "" {
		log.Fatalf("Managed Postgres required; use --data-directory")
	}

	pgConfig, err := postgres.ManagedConfig(dataDirectory)
	if err != nil {
		log.Fatal(err)
	}
	log.Println(pgConfig.Host)

	pg, err := postgres.NewPgDatabase(pgConfig)
	if err != nil {
		log.Fatal(err)
	}

	if len(flagset.Args()) == 0 {
		log.Fatal("No input files provided.")
	} else if len(flagset.Args()) > 1 {
		log.Fatal("Only one input file allowed.")
	}

	reader, err := evereader.NewFollowingReader(flagset.Args()[0])
	if err != nil {
		log.Fatal(err)
	}

	//indexer := postgres.NewPgEventIndexer(pg)

	count := 0

	submitChan := make(chan eve.EveEvent)

	wg := sync.WaitGroup{}
	threadCount := runtime.NumCPU()

	for i := 0; i < threadCount; i++ {
		thread := i
		wg.Add(1)
		go func() {
			_count := 0
			_indexer := postgres.NewPgEventIndexer(pg)
			for event := range submitChan {
				if event == nil {
					break
				}
				_indexer.Submit(event)
				_count++
				if _count == 1000 {
					_indexer.Commit()
					_count = 0
				}
			}
			_indexer.Commit()
			log.Info("Thread %d returning.", thread)
			wg.Done()
		}()
	}

	for {
		eof := false
		event, err := reader.Next()
		if err != nil {
			if err == io.EOF {
				eof = true
			} else {
				log.Fatal(err)
			}
		}

		if event != nil {
			if useNow {
				event.SetTimestamp(time.Now())
			}
			submitChan <- event
			count++
		}

		if eof {
			break
		}
	}

	log.Info("Closing down channels.")
	for i := 0; i < threadCount; i++ {
		submitChan <- nil
	}

	wg.Wait()

	log.Println(count)

	//log.Println("Committing.")
	//indexer.Commit()
	//log.Info("Committed %d events.", count)
}

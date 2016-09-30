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

package eveimport

import (
	"fmt"
	"github.com/jasonish/evebox/elasticsearch"
	"github.com/jasonish/evebox/evereader"
	flag "github.com/spf13/pflag"
	"os"
	"io"
	"time"
	"github.com/jasonish/evebox/log"
)

var flagset *flag.FlagSet

func usage() {
	usage := `Usage: evebox import [options] /path/to/eve.json

Options:
`
	fmt.Fprint(os.Stderr, usage)
	flagset.PrintDefaults()
}

func Main(args []string) {

	var elasticSearchUri string
	var oneshot bool
	var index string
	var verbose bool

	flagset = flag.NewFlagSet("import", flag.ExitOnError)
	flagset.Usage = usage
	flagset.StringVarP(&elasticSearchUri, "elasticsearch", "e", "", "Elastic Search URL")
	flagset.BoolVar(&oneshot, "oneshot", false, "One shot mode (exit on EOF)")
	flagset.StringVar(&index, "index", "evebox", "Elastic Search index prefix")
	flagset.BoolVarP(&verbose, "verbose", "v", false, "Verbose output")
	flagset.Parse(args[1:])

	if verbose {
		log.SetLevel(log.DEBUG)
	}

	if elasticSearchUri == "" {
		log.Fatal("error: --elasticsearch is a required parameter")
	}

	if len(flagset.Args()) == 0 {
		log.Fatal("error: no input file provided")
	}

	es := elasticsearch.New(elasticSearchUri)
	response, err := es.Ping()
	if err != nil {
		log.Fatal("error: failed to ping Elastic Search:", err)
	}
	log.Info("Connected to Elastic Search v%s (cluster:%s; name: %s)",
		response.Version.Number, response.ClusterName, response.Name)

	// Check if the template exists.
	templateExists, err := es.CheckTemplate(index)
	if !templateExists {
		log.Info("Template %s does not exist, creating...", index)
		err = es.LoadTemplate(index)
		if err != nil {
			log.Fatal("Failed to create template:", err)
		}
	} else {
		log.Info("Template %s exists, will not create.", index)
	}

	inputFiles := flagset.Args()

	indexer := elasticsearch.NewIndexer(es)
	indexer.IndexPrefix = index

	reader, err := evereader.New(inputFiles[0])
	if err != nil {
		log.Fatal(err)
	}

	count := 0

	go func() {
		err := indexer.Run()
		if err != nil {
			log.Fatal("Elastic Search indexer connection unexpectedly closed:", err)
		} else {
			log.Println("Indexer exited without issue.")
		}
	}()

	for {
		event, err := reader.Next()
		if err != nil {
			if err == io.EOF {
				if oneshot {
					indexer.Stop()
					log.Println("EOF: Exiting due to --oneshot.")
					break
				}
				// Flush the connection and sleep for a moment.
				log.Println("Got EOF. Flushing...")
				indexer.FlushConnection()
				time.Sleep(1 * time.Second)
				continue
			} else {
				log.Fatal(err)
			}
		}

		if event == nil {
			log.Fatal("Unexpected nil event: err=", err)
		}

		indexer.IndexRawEvent(event)

		count++

		if count > 0 && count % 1000 == 0 {
			response, err := indexer.FlushConnection()
			if err != nil {
				log.Fatal(err)
			}
			log.Debug("Indexed %d events {errors=%v}", len(response.Items),
				response.Errors)
		}
	}

}

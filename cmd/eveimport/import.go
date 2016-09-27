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
	"log"
	"os"
)

var flagset *flag.FlagSet

func usage() {
	usage := `Usage: evebox import [options] /path/to/eve.json

Options:
`
	fmt.Fprint(os.Stderr, usage)
	flagset.PrintDefaults()
}

func ImportFile(es *elasticsearch.ElasticSearch, filename string) error {

	reader, err := evereader.New(filename)
	if err != nil {
		return err
	}

	for {
		event, err := reader.Next()
		if err != nil {
			return err
		}

		es.IndexRawEveEvent(event)
	}

	return nil
}

func Main(args []string) {

	var elasticSearchUri string

	flagset = flag.NewFlagSet("import", flag.ExitOnError)
	flagset.Usage = usage
	flagset.StringVarP(&elasticSearchUri, "elasticsearch", "e", "", "Elastic Search URL")
	flagset.Parse(args[1:])

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
	log.Printf("Connected to Elastic Search v%s (cluster:%s; name: %s)",
		response.Version.Number, response.ClusterName, response.Name)

	inputFiles := flagset.Args()

	for i := 0; i < len(inputFiles); i++ {
		log.Println("Importing", inputFiles[i])
		err = ImportFile(es, inputFiles[i])
		if err != nil {
			log.Fatal(err)
		}
	}
}

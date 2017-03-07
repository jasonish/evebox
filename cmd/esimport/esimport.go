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

package esimport

import (
	"encoding/json"
	"fmt"
	"github.com/jasonish/evebox/elasticsearch"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/evereader"
	"github.com/jasonish/evebox/geoip"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/useragent"
	"github.com/spf13/pflag"
	"github.com/spf13/viper"
	"io"
	"os"
	"time"
)

const DEFAULT_INDEX = "evebox"
const BATCH_SIZE = 1000

var flagset *pflag.FlagSet

var verbose = false
var stdout = false
var oneshot = false

func usage() {
	usage := `Usage: evebox esimport [options] /path/to/eve.json

Options:
`
	fmt.Fprint(os.Stderr, usage)
	flagset.PrintDefaults()
}

func configure(args []string) {

	viper.SetDefault("index", DEFAULT_INDEX)
	viper.SetDefault("disable-certificate-check", false)
	viper.SetDefault("geoip-enabled", true)

	flagset = pflag.NewFlagSet("esimport", pflag.ExitOnError)
	flagset.Usage = usage

	configFilename := flagset.StringP("config", "c", "", "Configuration file")

	flagset.BoolVarP(&verbose, "verbose", "v", false, "Verbose output")
	flagset.BoolVar(&oneshot, "oneshot", false, "One shot mode (exit on EOF)")
	flagset.BoolVar(&stdout, "stdout", false, "Print events to stdout")

	flagset.StringP("elasticsearch", "e", "", "Elastic Search URL")
	viper.BindPFlag("elasticsearch", flagset.Lookup("elasticsearch"))

	flagset.StringP("username", "u", "", "Username")
	viper.BindPFlag("username", flagset.Lookup("username"))

	flagset.StringP("password", "p", "", "Password")
	viper.BindPFlag("password", flagset.Lookup("password"))

	flagset.BoolP("no-check-certificate", "k", false, "Disable certificate check")
	viper.BindPFlag("disable-certificate-check", flagset.Lookup("no-check-certificate"))

	flagset.String("index", DEFAULT_INDEX, "Elastic Search index prefix")
	viper.BindPFlag("index", flagset.Lookup("index"))

	flagset.Bool("end", false, "Start at end of file")
	viper.BindPFlag("end", flagset.Lookup("end"))

	flagset.Bool("bookmark", false, "Enable bookmarking")
	viper.BindPFlag("bookmark", flagset.Lookup("bookmark"))

	flagset.String("bookmark-path", "", "Path to bookmark file")
	viper.BindPFlag("bookmark-path", flagset.Lookup("bookmark-path"))

	flagset.String("geoip-database", "", "Path to GeoIP (v2) database file")
	viper.BindPFlag("geoip.database", flagset.Lookup("geoip-database"))

	flagset.Parse(args[1:])

	if *configFilename != "" {
		log.Info("Using configuration file %s", *configFilename)
		viper.SetConfigFile(*configFilename)
		if err := viper.ReadInConfig(); err != nil {
			log.Fatal(err)
		}
	}

	if verbose {
		log.Info("Setting log level to debug")
		log.SetLevel(log.DEBUG)
	}

	if len(flagset.Args()) == 1 {
		viper.Set("input", flagset.Args()[0])
	} else if len(flagset.Args()) > 1 {
		log.Fatal("Multiple input filenames not allowed")
	}
}

func Main(args []string) {

	configure(args)

	if viper.GetString("elasticsearch") == "" {
		log.Error("error: --elasticsearch is a required parameter")
		usage()
		os.Exit(1)
	}

	if viper.GetString("input") == "" {
		log.Fatal("error: no input file provided")
	}

	useBookmark := viper.GetBool("bookmark")
	bookmarkPath := viper.GetString("bookmark-path")

	if useBookmark && bookmarkPath == "" {
		bookmarkPath = fmt.Sprintf("%s.bookmark", viper.GetString("input"))
		log.Info("Using bookmark file %s", bookmarkPath)
	}

	es := elasticsearch.New(viper.GetString("elasticsearch"))
	es.EventBaseIndex = viper.GetString("index")
	es.DisableCertCheck(viper.GetBool("disable-certificate-check"))
	if viper.GetString("username") != "" || viper.GetString("password") != "" {
		if err := es.SetUsernamePassword(viper.GetString("username"),
			viper.GetString("password")); err != nil {
			log.Fatal("Failed to set username and password: %v", err)
		}
	}
	response, err := es.Ping()
	if err != nil {
		log.Fatal("error: failed to ping Elastic Search:", err)
	}
	log.Info("Connected to Elastic Search v%s (cluster:%s; name: %s)",
		response.Version.Number, response.ClusterName, response.Name)
	majorVersion := response.MajorVersion()

	// Check if the template exists.
	templateExists, err := es.CheckTemplate(es.EventBaseIndex)
	if !templateExists {
		log.Info("Template %s does not exist, creating...", es.EventBaseIndex)
		err = es.LoadTemplate(es.EventBaseIndex, majorVersion)
		if err != nil {
			log.Fatal("Failed to create template:", err)
		}
	} else {
		log.Info("Template %s exists, will not create.", es.EventBaseIndex)
	}

	geoIpFilter := eve.NewGeoipFilter(geoip.NewGeoIpService())

	indexer := elasticsearch.NewIndexer(es)

	reader, err := evereader.New(viper.GetString("input"))
	if err != nil {
		log.Fatal(err)
	}

	// Initialize bookmarking...
	var bookmarker *evereader.Bookmarker = nil
	optEnd := viper.GetBool("end")
	if useBookmark {
		bookmarker = &evereader.Bookmarker{
			Filename: bookmarkPath,
			Reader:   reader,
		}
		err := bookmarker.Init(optEnd)
		if err != nil {
			log.Fatal(err)
		}
	} else if optEnd {
		log.Info("Jumping to end of file.")
		err := reader.SkipToEnd()
		if err != nil {
			log.Fatal(err)
		}
	}

	uaFilter := useragent.EveUserAgentFilter{}
	tagsFilter := eve.TagsFilter{}

	count := uint64(0)
	lastStatTs := time.Now()
	lastStatCount := uint64(0)
	startTime := time.Now()

	// Number of EOFs in last stat interval.
	eofs := uint64(0)

	for {
		eof := false
		event, err := reader.Next()
		if err != nil {
			if err == io.EOF {
				eof = true
				eofs++
			} else if _, ok := err.(evereader.MalformedEventError); ok {
				log.Error("Failed to read event but will continue: %v", err)
			} else {
				log.Fatalf("Unrecoverable error reading event: %v", err)
			}
		}

		if event != nil {

			geoIpFilter.Filter(event)
			tagsFilter.Filter(event)
			uaFilter.Filter(event)

			if stdout {
				asJson, err := json.Marshal(event)
				if err != nil {
					log.Error("Failed to print event as json: %v", err)
				} else {
					fmt.Println(string(asJson))
				}
			}

			indexer.Submit(event)
			count++
		}

		if eof || (count > 0 && count%BATCH_SIZE == 0) {
			var bookmark *evereader.Bookmark = nil

			if useBookmark {
				bookmark = bookmarker.GetBookmark()
			}

			status, err := indexer.Commit()
			if err != nil {
				log.Fatal(err)
			}
			if status != nil {
				response, ok := status.(*elasticsearch.BulkResponse)
				if ok {
					log.Debug("Indexed %d events {errors=%v}", len(response.Items),
						response.Errors)
				}
			}

			if useBookmark {
				bookmarker.WriteBookmark(bookmark)
			}
		}

		now := time.Now()
		if now.Sub(lastStatTs).Seconds() > 1 && now.Second() == 0 {

			// Calculate the lag in bytes, that is the number of bytes behind
			// the end of file we are.
			lag, err := reader.Lag()
			if err != nil {
				log.Error("Failed to calculate lag: %v", err)
			}

			log.Info("Total: %d; Last minute: %d; Avg: %.2f/s, EOFs: %d; Lag (bytes): %d",
				count,
				count-lastStatCount,
				float64(count-lastStatCount)/(now.Sub(lastStatTs).Seconds()),
				eofs,
				lag)
			lastStatTs = now
			lastStatCount = count
			eofs = 0
		}

		if eof {
			if oneshot {
				break
			} else {
				time.Sleep(1 * time.Second)
			}
		}
	}

	totalTime := time.Since(startTime)

	if oneshot {
		log.Info("Indexed %d events: time=%.2fs; avg=%d/s", count, totalTime.Seconds(),
			uint64(float64(count)/totalTime.Seconds()))
	}
}

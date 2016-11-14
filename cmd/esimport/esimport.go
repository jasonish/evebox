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
	"github.com/jasonish/evebox/config"
	"github.com/jasonish/evebox/elasticsearch"
	"github.com/jasonish/evebox/evereader"
	"github.com/jasonish/evebox/geoip"
	"github.com/jasonish/evebox/log"
	flag "github.com/spf13/pflag"
	"io"
	"os"
	"time"
	"github.com/jasonish/evebox/eve"
)

type Config struct {
	// The filename to read.
	InputFilename           string `yaml:"input"`

	// Elastic Search URL.
	Url                     string `yaml:"url"`

	// Elastic Search index (prefix)
	Index                   string `yaml:"index"`

	DisableCertificateCheck bool `yaml:"disable-certificate-check"`

	Username                string `yaml:"username"`
	Password                string `yaml:"password"`

	Bookmark                bool   `yaml:"bookmark"`
	BookmarkPath            string `yaml:"bookmark-path"`

	DisableGeoIp            bool   `yaml:"disable-geoip"`
	GeoIpDatabase           string `yaml:"geoip-database"`

	Verbose                 bool `yaml:"verbose"`

	End                     bool `yaml:"end"`

	BatchSize               uint64 `yaml:"batch-size"`

	// Not exposed in configuration file.
	stdout                  bool
	oneshot                 bool
}

type ConfigWrapper struct {
	Config Config `yaml:"esimport"`
}

var flagset *flag.FlagSet

func usage() {
	usage := `Usage: evebox import [options] /path/to/eve.json

Options:
`
	fmt.Fprint(os.Stderr, usage)
	flagset.PrintDefaults()
}

func configure(args []string) Config {
	flagset = flag.NewFlagSet("import", flag.ExitOnError)
	flagset.Usage = usage

	configFilename := flagset.StringP("config", "c", "", "Configuration file")
	verbose := flagset.BoolP("verbose", "v", false, "Verbose output")
	elasticSearchUri := flagset.StringP("elasticsearch", "e", "", "Elastic Search URL")
	username := flagset.StringP("username", "u", "", "Username")
	password := flagset.StringP("password", "p", "", "Password")
	noCheckCertificate := flagset.BoolP("no-check-certificate", "k", false, "Disable certificate check")
	index := flagset.String("index", "evebox", "Elastic Search index prefix")
	oneshot := flagset.Bool("oneshot", false, "One shot mode (exit on EOF)")
	stdout := flagset.Bool("stdout", false, "Print events to stdout")
	end := flagset.Bool("end", false, "Start at end of file")
	batchSize := flagset.Uint64("batch-size", 1000, "Batch import size")
	useBookmark := flagset.Bool("bookmark", false, "Bookmark location")
	bookmarkPath := flagset.String("bookmark-path", "", "Path to bookmark file")
	noGeoIp := flagset.Bool("no-geoip", false, "Disable GeoIP lookups")
	geoIpDatabase := flagset.String("geoip-database", "", "Path to GeoIP (v2) database file")

	flagset.Parse(args[1:])

	if *verbose {
		log.Info("Setting log level to debug")
		log.SetLevel(log.DEBUG)
	}

	configWrapper := ConfigWrapper{}
	configWrapper.Config.BatchSize = 1000

	if *configFilename != "" {
		log.Debug("Loading configuration file %s", *configFilename)
		err := config.LoadConfigTo(*configFilename, &configWrapper)
		if err != nil {
			log.Fatal(err)
		}
	}
	conf := configWrapper.Config

	flagset.Visit(func(flag *flag.Flag) {
		log.Debug("Found command line argument %s -> %s", flag.Name,
			flag.Value.String())
		switch flag.Name {
		case "elasticsearch":
			conf.Url = *elasticSearchUri
		case "username":
			conf.Username = *username
		case "password":
			conf.Password = *password
		case "no-check-certificate":
			conf.DisableCertificateCheck = *noCheckCertificate
		case "index":
			conf.Index = *index
		case "oneshot":
			conf.oneshot = *oneshot
		case "stdout":
			conf.stdout = *stdout
		case "end":
			conf.End = *end
		case "batch-size":
			conf.BatchSize = *batchSize
		case "bookmark":
			conf.Bookmark = *useBookmark
		case "bookmark-path":
			conf.BookmarkPath = *bookmarkPath
		case "no-geoip":
			conf.DisableGeoIp = *noGeoIp
		case "geoip-database":
			conf.GeoIpDatabase = *geoIpDatabase
		case "verbose":
			conf.Verbose = *verbose
		case "config":
		default:
			log.Notice("Unhandle configuration flag %s", flag.Name)
		}
	})

	if len(flagset.Args()) == 1 {
		conf.InputFilename = flagset.Args()[0]
	} else if len(flagset.Args()) > 1 {
		log.Fatal("Multiple input filenames not allowed")
	}

	return conf
}

func Main(args []string) {

	conf := configure(args)

	if conf.BatchSize < 1 {
		log.Fatal("Batch size must be greater than 0")
	}

	if conf.Url == "" {
		log.Error("error: --elasticsearch is a required parameter")
		usage()
		os.Exit(1)
	}

	if conf.InputFilename == "" {
		log.Fatal("error: no input file provided")
	}

	if conf.Bookmark && conf.BookmarkPath == "" {
		conf.BookmarkPath = fmt.Sprintf("%s.bookmark", conf.InputFilename)
		log.Info("Using bookmark file %s", conf.BookmarkPath)
	}

	es := elasticsearch.New(conf.Url)
	es.DisableCertCheck(conf.DisableCertificateCheck)
	if conf.Username != "" || conf.Password != "" {
		if err := es.SetUsernamePassword(conf.Username,
			conf.Password); err != nil {
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
	templateExists, err := es.CheckTemplate(conf.Index)
	if !templateExists {
		log.Info("Template %s does not exist, creating...", conf.Index)
		err = es.LoadTemplate(conf.Index, majorVersion)
		if err != nil {
			log.Fatal("Failed to create template:", err)
		}
	} else {
		log.Info("Template %s exists, will not create.", conf.Index)
	}

	var geoipFilter *eve.GeoipFilter

	if !conf.DisableGeoIp {
		geoipdb, err := geoip.NewGeoIpDb(conf.GeoIpDatabase)
		if err != nil {
			log.Notice("Failed to load GeoIP database: %v", err)
		} else {
			log.Info("Using GeoIP database %s, %s", geoipdb.Type(), geoipdb.BuildDate())
			geoipFilter = eve.NewGeoipFilter(geoipdb)
		}
	}

	indexer := elasticsearch.NewIndexer(es, conf.DisableCertificateCheck)
	indexer.IndexPrefix = conf.Index

	reader, err := evereader.New(conf.InputFilename)
	if err != nil {
		log.Fatal(err)
	}

	// Initialize bookmarking...
	var bookmarker *evereader.Bookmarker = nil
	if conf.Bookmark {
		bookmarker = &evereader.Bookmarker{
			Filename: conf.BookmarkPath,
			Reader:   reader,
		}
		err := bookmarker.Init(conf.End)
		if err != nil {
			log.Fatal(err)
		}
	} else if conf.End {
		log.Info("Jumping to end of file.")
		err := reader.SkipToEnd()
		if err != nil {
			log.Fatal(err)
		}
	}

	count := uint64(0)
	lastStatTs := time.Now()
	lastStatCount := uint64(0)
	startTime := time.Now()

	// Number of EOFs in last stat interval.
	eofs := uint64(0)

	go func() {
		err := indexer.Run()
		if err != nil {
			log.Fatal("Elastic Search indexer connection unexpectedly closed:", err)
		} else {
			log.Debug("Indexer exited without issue.")
		}
	}()

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

			if geoipFilter != nil {
				geoipFilter.AddGeoIP(event)
			}

			if conf.stdout {
				asJson, err := json.Marshal(event)
				if err != nil {
					log.Error("Failed to print event as json: %v", err)
				} else {
					fmt.Println(string(asJson))
				}
			}

			indexer.IndexRawEvent(event)
			count++
		}

		if eof || (count > 0 && count % conf.BatchSize == 0) {
			var bookmark *evereader.Bookmark = nil

			if conf.Bookmark {
				bookmark = bookmarker.GetBookmark()
			}

			response, err := indexer.FlushConnection()
			if err != nil {
				log.Fatal(err)
			}
			if response != nil {
				log.Debug("Indexed %d events {errors=%v}", len(response.Items),
					response.Errors)
			}

			if conf.Bookmark {
				bookmarker.WriteBookmark(bookmark)
			}
		}

		now := time.Now()
		if now.Sub(lastStatTs).Seconds() > 1 && now.Second() == 0 {

			// Calculate the lag in bytes, that is the number of bytes behind
			// the end of file we are.
			lag, err := GetLag(reader)
			if err != nil {
				log.Error("Failed to calculate lag: %v", err)
			}

			log.Info("Total: %d; Last minute: %d; Avg: %.2f/s, EOFs: %d; Lag (bytes): %d",
				count,
				count - lastStatCount,
				float64(count - lastStatCount) / (now.Sub(lastStatTs).Seconds()),
				eofs,
				lag)
			lastStatTs = now
			lastStatCount = count
			eofs = 0
		}

		if eof {
			if conf.oneshot {
				break
			} else {
				time.Sleep(1 * time.Second)
			}
		}
	}

	totalTime := time.Since(startTime)

	if conf.oneshot {
		log.Info("Indexed %d events: time=%.2fs; avg=%d/s", count, totalTime.Seconds(),
			uint64(float64(count) / totalTime.Seconds()))
	}
}

func GetLag(reader *evereader.EveReader) (int64, error) {
	fileSize, err := reader.FileSize()
	if err != nil {
		return 0, err
	}
	fileOffset, err := reader.FileOffset()
	if err != nil {
		return 0, err
	}
	return fileSize - fileOffset, nil
}

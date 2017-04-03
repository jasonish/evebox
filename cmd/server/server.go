/* Copyright (c) 2014-2015 Jason Ish
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

package server

import (
	"fmt"

	"github.com/jasonish/evebox/appcontext"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/elasticsearch"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/evereader"
	"github.com/jasonish/evebox/geoip"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/server"
	"github.com/jasonish/evebox/sqlite"
	"github.com/jasonish/evebox/useragent"
	"github.com/spf13/pflag"
	"github.com/spf13/viper"
)

const DEFAULT_DATA_DIR = "."
const DEFAULT_ELASTICSEARCH_URL string = "http://localhost:9200"
const DEFAULT_ELASTICSEARCH_INDEX string = "logstash"

var opts struct {
	Port               string
	Host               string
	DevServerUri       string
	Version            bool
	NoCheckCertificate bool
}

func VersionMain() {
	fmt.Printf("EveBox Version %s (rev %s)\n",
		core.BuildVersion, core.BuildRev)
}

func initViper() {
	viper.SetDefault("data-directory", DEFAULT_DATA_DIR)
	viper.SetDefault("elasticsearch", DEFAULT_ELASTICSEARCH_URL)
	viper.SetDefault("index", DEFAULT_ELASTICSEARCH_INDEX)

	// Retention period in days.
	viper.SetDefault("database.retention-period", 0)
	viper.BindEnv("database.retention-period", "RETENTION_PERIOD")

	viper.BindEnv("input.bookmark-directory", "BOOKMARK_DIRECTORY")
}

func getElasticSearchKeyword(flagset *pflag.FlagSet) (bool, string) {
	flag := flagset.Lookup("elasticsearch-keyword")
	if flag.Changed {
		return true, flag.Value.String()
	}

	if viper.IsSet("database.elasticsearch.keyword") {
		return true, viper.GetString("database.elasticsearch.keyword")
	}

	return false, ""
}

func Main(args []string) {

	log.SetLevel(log.INFO)

	var configFilename string
	var err error
	verbose := false

	log.Info("This is EveBox Server version %v (rev: %v)", core.BuildVersion, core.BuildRev)

	initViper()

	flagset := pflag.NewFlagSet("server", pflag.ExitOnError)

	// Datastore type.
	flagset.String("datastore", "elasticsearch", "Datastore to use")
	viper.BindPFlag("database.type", flagset.Lookup("datastore"))
	viper.BindEnv("database.type", "DATABASE_TYPE")

	flagset.StringP("elasticsearch", "e", DEFAULT_ELASTICSEARCH_URL, "Elastic Search URI (default: http://localhost:9200")
	viper.BindPFlag("database.elasticsearch.url", flagset.Lookup("elasticsearch"))
	viper.BindEnv("database.elasticsearch.url", "ELASTICSEARCH_URL")

	flagset.StringP("index", "i", DEFAULT_ELASTICSEARCH_INDEX, "Elastic Search Index (default: logstash)")
	viper.BindPFlag("database.elasticsearch.index", flagset.Lookup("index"))
	viper.BindEnv("index", "ELASTICSEARCH_INDEX")

	// Elastic Search keyword. This is purposely not bound to viper as viper
	// fails to tell us if it was set or not.
	flagset.String("elasticsearch-keyword", "", "Elastic Search keyword")

	flagset.BoolVarP(&verbose, "verbose", "v", false, "Verbose (debug logging)")

	flagset.StringVarP(&opts.Port, "port", "p", "5636", "Port to bind to")
	flagset.StringVarP(&opts.Host, "host", "", "0.0.0.0", "Host to bind to")
	flagset.StringVarP(&opts.DevServerUri, "dev", "", "", "Frontend development server URI")
	flagset.BoolVarP(&opts.Version, "version", "", false, "Show version")
	flagset.StringVarP(&configFilename, "config", "c", "", "Configuration filename")
	flagset.BoolVarP(&opts.NoCheckCertificate, "no-check-certificate", "k", false, "Disable certificate check for Elastic Search")

	flagset.StringP("data-directory", "D", DEFAULT_DATA_DIR, "Data directory")
	viper.BindPFlag("data-directory", flagset.Lookup("data-directory"))
	viper.BindEnv("data-directory", "DATA_DIRECTORY")

	var input string
	flagset.StringVar(&input, "input", "", "Input eve-log file (optional)")

	flagset.Parse(args[0:])

	if opts.Version {
		VersionMain()
		return
	}

	if verbose {
		log.SetLevel(log.DEBUG)
	}

	if configFilename != "" {
		viper.SetConfigFile(configFilename)
	} else {
		viper.SetConfigName("evebox")
		viper.SetConfigType("yaml")
		viper.AddConfigPath(".")
	}
	if err := viper.ReadInConfig(); err != nil {
		if configFilename != "" {
			log.Fatal(err)
		}
	}

	if input != "" {
		viper.Set("input.enabled", "true")
		viper.Set("input.filename", input)
	}

	appContext := appcontext.AppContext{}
	appContext.GeoIpService = geoip.NewGeoIpService()
	appContext.Vars.DevWebAppServerUrl = opts.DevServerUri

	switch viper.GetString("database.type") {
	case "elasticsearch":
		log.Info("Configuring ElasticSearch datastore")
		log.Info("Using ElasticSearch URL %s",
			viper.GetString("database.elasticsearch.url"))
		log.Info("Using ElasticSearch Index %s.",
			viper.GetString("database.elasticsearch.index"))
		elasticSearch := elasticsearch.New(
			viper.GetString("database.elasticsearch.url"))
		elasticSearch.SetEventIndex(
			viper.GetString("database.elasticsearch.index"))

		keywordSet, keyword := getElasticSearchKeyword(flagset)
		if keywordSet {
			log.Info("Forcing Elastic Search keyword to '%s'", keyword)
			elasticSearch.SetKeyword(keyword)
		} else {
			elasticSearch.InitKeyword()
		}

		pingResponse, err := elasticSearch.Ping()
		if err != nil {
			log.Error("Failed to ping Elastic Search: %v", err)
		} else {
			log.Info("Connected to Elastic Search (version: %s)",
				pingResponse.Version.Number)
			major, _ := pingResponse.ParseVersion()
			if major < 5 {
				log.Warning("Elastic Search versions less than 5 will be unsupported in a future release")
			}
		}
		appContext.ElasticSearch = elasticSearch
		appContext.EventService = elasticsearch.NewEventService(elasticSearch)
		appContext.ReportService = elasticsearch.NewReportService(elasticSearch)
		appContext.DataStore, err = elasticsearch.NewDataStore(elasticSearch)
		if err != nil {
			log.Fatal(err)
		}
		appContext.SetFeature(core.FEATURE_REPORTING)
	case "sqlite":
		if err := sqlite.InitSqlite(&appContext); err != nil {
			log.Fatal(err)
		}
	default:
		log.Fatal("unsupported datastore: ",
			viper.GetString("database.type"))
	}

	initInternalEveReader(&appContext)

	httpServer := server.NewServer(appContext)
	err = httpServer.Start(opts.Host + ":" + opts.Port)
	if err != nil {
		log.Fatal(err)
	}
}

func initInternalEveReader(appContext *appcontext.AppContext) {
	enabled := viper.GetBool("input.enabled")
	if !enabled {
		return
	}
	log.Info("Configuring internal eve log reader")
	filename := viper.GetString("input.filename")
	bookmarkDirectory := viper.GetString("input.bookmark-directory")

	eveFileProcessor := &evereader.EveFileProcessor{
		Filename:          filename,
		BookmarkDirectory: bookmarkDirectory,
		Sink:              appContext.DataStore.GetEveEventSink(),
	}

	eveFileProcessor.AddFilter(&eve.TagsFilter{})
	eveFileProcessor.AddFilter(eve.NewGeoipFilter(appContext.GeoIpService))
	eveFileProcessor.AddFilter(&useragent.EveUserAgentFilter{})

	for field, value := range viper.GetStringMap("input.custom-fields") {
		eveFileProcessor.AddCustomField(field, value)
	}

	eveFileProcessor.Start()
}

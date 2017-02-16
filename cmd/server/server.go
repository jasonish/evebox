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

	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/elasticsearch"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/server"
	"github.com/jasonish/evebox/sqlite"
	flag "github.com/spf13/pflag"
	"github.com/spf13/viper"
)

const DEFAULT_ELASTICSEARCH_URL string = "http://localhost:9200"
const DEFAULT_ELASTICSEARCH_INDEX string = "logstash"

var opts struct {
	Port               string
	Host               string
	DevServerUri       string
	Version            bool
	NoCheckCertificate bool

	// If true, use SQLite, otherwise use Elastic Search.
	Sqlite bool
}

func VersionMain() {
	fmt.Printf("EveBox Version %s (rev %s) [%s]\n",
		core.BuildVersion, core.BuildRev, core.BuildDate)
}

func setDefaults() {
	viper.SetDefault("elasticsearch", DEFAULT_ELASTICSEARCH_URL)
	viper.SetDefault("index", DEFAULT_ELASTICSEARCH_INDEX)
}

func Main(args []string) {

	var configFilename string
	var err error

	log.Info("This is EveBox Server version %v (rev: %v)", core.BuildVersion, core.BuildRev)

	setDefaults()

	flagset := flag.NewFlagSet("server", flag.ExitOnError)

	flagset.StringP("elasticsearch", "e", DEFAULT_ELASTICSEARCH_URL, "Elastic Search URI (default: http://localhost:9200")
	viper.BindPFlag("elasticsearch", flagset.Lookup("elasticsearch"))
	viper.BindEnv("elasticsearch", "ELASTICSEARCH_URL")

	flagset.StringP("index", "i", DEFAULT_ELASTICSEARCH_INDEX, "Elastic Search Index (default: logstash)")
	viper.BindPFlag("index", flagset.Lookup("index"))
	viper.BindEnv("index", "ELASTICSEARCH_INDEX")

	flagset.StringVarP(&opts.Port, "port", "p", "5636", "Port to bind to")
	flagset.StringVarP(&opts.Host, "host", "", "0.0.0.0", "Host to bind to")
	flagset.StringVarP(&opts.DevServerUri, "dev", "", "", "Frontend development server URI")
	flagset.BoolVarP(&opts.Version, "version", "", false, "Show version")
	flagset.StringVarP(&configFilename, "config", "c", "", "Configuration filename")
	flagset.BoolVarP(&opts.NoCheckCertificate, "no-check-certificate", "k", false, "Disable certificate check for Elastic Search")

	flagset.BoolVarP(&opts.Sqlite, "sqlite", "", false, "Use SQLite for the event store")

	flagset.Parse(args[0:])

	if opts.Version {
		VersionMain()
		return
	}

	log.SetLevel(log.DEBUG)

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

	log.Info("Using ElasticSearch URL %s", viper.GetString("elasticsearch"))
	log.Info("Using ElasticSearch Index %s.", viper.GetString("index"))

	appContext := server.AppContext{}
	elasticSearch := elasticsearch.New(viper.GetString("elasticsearch"))
	elasticSearch.SetEventIndex(viper.GetString("index"))
	elasticSearch.InitKeyword()
	pingResponse, err := elasticSearch.Ping()
	if err != nil {
		log.Error("Failed to ping Elastic Search: %v", err)
	} else {
		log.Info("Connected to Elastic Search (version: %s)",
			pingResponse.Version.Number)
	}
	appContext.ElasticSearch = elasticSearch
	appContext.EventService = elasticsearch.NewEventService(elasticSearch)
	appContext.AlertQueryService = elasticsearch.NewAlertQueryService(elasticSearch)
	appContext.EventQueryService = elasticsearch.NewEventQueryService(elasticSearch)
	appContext.ReportService = elasticsearch.NewReportService(elasticSearch)

	appContext.Vars.DevWebAppServerUrl = opts.DevServerUri

	var datastoreType string = "elasticsearch"
	if opts.Sqlite {
		datastoreType = "sqlite"
	}

	if datastoreType == "elasticsearch" {
		appContext.DataStore, err = elasticsearch.NewDataStore(elasticSearch)
		if err != nil {
			log.Fatal(err)
		}
	} else if datastoreType == "sqlite" {
		appContext.DataStore, err = sqlite.NewDataStore()
		if err != nil {
			log.Fatal(err)
		}
	}

	httpServer := server.NewServer(appContext)
	err = httpServer.Start(opts.Host + ":" + opts.Port)
	if err != nil {
		log.Fatal(err)
	}
}

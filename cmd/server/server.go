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
	"os"

	"github.com/jasonish/evebox/config"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/elasticsearch"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/server"
	"github.com/jasonish/evebox/sqlite"
	flag "github.com/spf13/pflag"
)

const DEFAULT_ELASTICSEARCH_URL string = "http://localhost:9200"

var opts struct {
	// We don't provide a default for this one so we can easily
	// detect if its been set or not.
	ElasticSearchUri   string
	ElasticSearchIndex string
	Port               string
	Host               string
	DevServerUri       string
	Version            bool
	Config             string
	NoCheckCertificate bool
}

var conf *config.Config

func init() {
	conf = config.NewConfig()
}

func VersionMain() {
	fmt.Printf("EveBox Version %s (rev %s) [%s]\n",
		core.BuildVersion, core.BuildRev, core.BuildDate)
}

func getElasticSearchUrl() string {
	if opts.ElasticSearchUri != "" {
		return opts.ElasticSearchUri
	}
	if os.Getenv("ELASTICSEARCH_URL") != "" {
		return os.Getenv("ELASTICSEARCH_URL")
	}
	return DEFAULT_ELASTICSEARCH_URL
}

func getElasticSearchIndex() string {
	if opts.ElasticSearchIndex != "" {
		return opts.ElasticSearchIndex
	} else if os.Getenv("ELASTICSEARCH_INDEX") != "" {
		return os.Getenv("ELASTICSEARCH_INDEX")
	} else {
		return "logstash"
	}
}

func Main(args []string) {

	var err error

	flagset := flag.NewFlagSet("server", flag.ExitOnError)

	flagset.StringVarP(&opts.ElasticSearchUri, "elasticsearch", "e", "", "Elastic Search URI (default: http://localhost:9200")
	flagset.StringVarP(&opts.ElasticSearchIndex, "index", "i", "", "Elastic Search Index (default: logstash)")
	flagset.StringVarP(&opts.Port, "port", "p", "5636", "Port to bind to")
	flagset.StringVarP(&opts.Host, "host", "", "0.0.0.0", "Host to bind to")
	flagset.StringVarP(&opts.DevServerUri, "dev", "", "", "Frontend development server URI")
	flagset.BoolVarP(&opts.Version, "version", "", false, "Show version")
	flagset.StringVarP(&opts.Config, "config", "c", "", "Configuration filename")
	flagset.BoolVarP(&opts.NoCheckCertificate, "no-check-certificate", "k", false, "Disable certificate check for Elastic Search")

	flagset.Parse(args[0:])

	if opts.Version {
		VersionMain()
		return
	}

	log.SetLevel(log.DEBUG)

	// If no configuration was provided, see if evebox.yaml exists
	// in the current directory.
	if opts.Config == "" {
		_, err = os.Stat("./evebox.yaml")
		if err == nil {
			opts.Config = "./evebox.yaml"
		}
	}
	if opts.Config != "" {
		log.Printf("Loading configuration file %s.\n", opts.Config)
		conf, err = config.LoadConfig(opts.Config)
		if err != nil {
			log.Fatal(err)
		}
	}

	conf.ElasticSearchIndex = getElasticSearchIndex()
	log.Info("Using ElasticSearch Index %s.", conf.ElasticSearchIndex)

	appContext := server.AppContext{
		Config: conf,
	}
	elasticSearch := elasticsearch.New(getElasticSearchUrl())
	elasticSearch.SetEventIndex(conf.ElasticSearchIndex)
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

	dataStoreType := "elasticsearch"
	//dataStoreType := "sqlite"

	if dataStoreType == "elasticsearch" {
		appContext.DataStore, err = elasticsearch.NewDataStore(elasticSearch)
		if err != nil {
			log.Fatal(err)
		}
	} else if dataStoreType == "sqlite" {
		appContext.DataStore, err = sqlite.NewDataStore()
		if err != nil {
			log.Fatal(err)
		}
	}

	router := server.NewRouter()
	httpServer := server.NewServer(appContext, router)
	httpServer.RegisterApiHandlers()

	// Static file server, must be last as it serves as the fallback.
	router.Prefix("/", server.StaticHandlerFactory(opts.DevServerUri))

	err = httpServer.Start(opts.Host + ":" + opts.Port)
	if err != nil {
		log.Fatal(err)
	}
}

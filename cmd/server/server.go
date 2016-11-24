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
	"github.com/jessevdk/go-flags"
)

const DEFAULT_ELASTICSEARCH_URL string = "http://localhost:9200"

var opts struct {
	// We don't provide a default for this one so we can easily
	// detect if its been set or not.
	ElasticSearchUri   string `long:"elasticsearch" short:"e" description:"Elastic Search URI (default: http://localhost:9200)"`
	ElasticSearchIndex string `long:"index" short:"i" description:"Elastic Search Index (default: logstash)"`
	Port               string `long:"port" short:"p" default:"5636" description:"Port to bind to"`
	Host               string `long:"host" default:"0.0.0.0" description:"Host to bind to"`
	DevServerUri       string `long:"dev" description:"Frontend development server URI"`
	Version            bool   `long:"version" description:"Show version"`
	Config             string `long:"config" short:"c" description:"Configuration filename"`
	NoCheckCertificate bool   `long:"no-check-certificate" short:"k" description:"Disable certificate check for Elastic Search"`
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

	_, err := flags.ParseArgs(&opts, args)
	if err != nil {
		// flags.Parse should have already presented an error message.
		os.Exit(1)
	}

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

	router := server.NewRouter()

	router.Handle("/api/1/archive",
		server.ApiF(appContext, server.ArchiveHandler))
	router.Handle("/api/1/escalate",
		server.ApiF(appContext, server.EscalateHandler))

	router.POST("/api/1/alert-group/add-tags",
		server.ApiF(appContext, server.AlertGroupAddTags))
	router.POST("/api/1/alert-group/remove-tags",
		server.ApiF(appContext, server.AlertGroupRemoveTags))

	router.Handle("/api/1/event/{id}",
		server.ApiF(appContext, server.GetEventByIdHandler))

	router.POST("/api/1/event/{id}/archive", server.ApiF(appContext, server.ArchiveEventHandler))
	router.POST("/api/1/event/{id}/escalate", server.ApiF(appContext, server.EscalateEventHandler))
	router.POST("/api/1/event/{id}/de-escalate", server.ApiF(appContext, server.DeEscalateEventHandler))

	router.Handle("/api/1/config",
		server.ApiF(appContext, server.ConfigHandler))
	router.Handle("/api/1/version",
		server.ApiF(appContext, server.VersionHandler))
	router.Handle("/api/1/eve2pcap", server.ApiF(appContext, server.Eve2PcapHandler))

	router.GET("/api/1/alerts", server.ApiF(appContext, server.AlertsHandler))
	router.GET("/api/1/event-query", server.ApiF(appContext, server.EventQueryHandler))

	router.Handle("/api/1/query", server.ApiF(appContext, server.QueryHandler))

	router.Handle("/api/1/_bulk", server.ApiF(appContext, server.EsBulkHandler))

	router.GET("/api/1/report/dns/requests/rrnames", server.ApiF(appContext, server.ReportDnsRequestRrnames))
	router.POST("/api/1/report/dns/requests/rrnames", server.ApiF(appContext, server.ReportDnsRequestRrnames))

	router.GET("/api/1/report/agg", server.ApiF(appContext, server.ReportAggs))
	router.GET("/api/1/report/histogram", server.ApiF(appContext, server.ReportHistogram))

	// /api/1/report/netflow/sources/bytes
	// /api/1/report/netflow/sources/packets

	// /api/1/report/netflow/destinations/bytes
	// /api/1/report/netflow/destinations/packets

	// This all needs some cleanup...

	httpServer := server.NewServer(appContext, router)
	httpServer.RegisterApiHandlers()

	// Static file server, must be last as it serves as the fallback.
	router.Prefix("/", server.StaticHandlerFactory(opts.DevServerUri))

	err = httpServer.Start(opts.Host + ":" + opts.Port)
	if err != nil {
		log.Fatal(err)
	}
}

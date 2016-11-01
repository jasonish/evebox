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
	"encoding/json"
	"fmt"
	"net/http"
	"os"

	"github.com/gorilla/mux"
	"github.com/jasonish/evebox"
	"github.com/jasonish/evebox/config"
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
	ElasticSearchIndex string `long:"index" short:"i" description:"Elastic Search Index (default: logstash-*)"`
	Port               string `long:"port" short:"p" default:"5636" description:"Port to bind to"`
	Host               string `long:"host" default:"0.0.0.0" description:"Host to bind to"`
	DevServerUri       string `long:"dev" description:"Frontend development server URI"`
	Version            bool   `long:"version" description:"Show version"`
	Config             string `long:"config" short:"c" description:"Configuration filename"`
	NoCheckCertificate bool   `long:"no-check-certificate" short:"k" description:"Disable certificate check for Elastic Search"`
}

var conf = &config.Config{}

type VersionResponse struct {
	Version  string
	Revision string
	Date     string
}

func VersionHandler(w http.ResponseWriter, r *http.Request) {
	response := VersionResponse{
		evebox.BuildVersion,
		evebox.BuildRev,
		evebox.BuildDate,
	}
	w.Header().Set("Content-Type", "application/json; charset=UTF-8")
	json.NewEncoder(w).Encode(response)
}

func ConfigHandler(appContext server.AppContext) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {

		// A bit of a hack here... Provide the keyword type in the config
		// object to the frontend doesn't have to do extra work to find it.
		if conf.Extra == nil {
			conf.Extra = make(map[string]interface{})
		}
		conf.Extra["elasticSearchKeyword"], _ = appContext.ElasticSearch.GetKeywordType("")

		configJson, err :=
			conf.ToJSON()
		if err != nil {
			// Return failure.
			log.Println("error: ", err)
			return
		}
		w.Header().Set("Content-Type", "application/json; charset=UTF-8")
		w.Write(configJson)
	})
}

func setupElasticSearchProxy(router *mux.Router) {
	if opts.ElasticSearchUri == "" {
		if os.Getenv("ELASTICSEARCH_URL") != "" {
			opts.ElasticSearchUri = os.Getenv("ELASTICSEARCH_URL")
		} else {
			opts.ElasticSearchUri = DEFAULT_ELASTICSEARCH_URL
		}
	}
	log.Printf("Elastic Search URL: %v", opts.ElasticSearchUri)
	esProxy, err := evebox.NewElasticSearchProxy(opts.ElasticSearchUri,
		"/elasticsearch", opts.NoCheckCertificate)
	if err != nil {
		log.Fatal(err)
	}
	router.PathPrefix("/elasticsearch").Handler(esProxy)
}

func VersionMain() {
	fmt.Printf("EveBox Version %s (rev %s) [%s]\n",
		evebox.BuildVersion, evebox.BuildRev, evebox.BuildDate)
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

	if opts.ElasticSearchIndex != "" {
		conf.ElasticSearchIndex = opts.ElasticSearchIndex
	} else if os.Getenv("ELASTICSEARCH_INDEX") != "" {
		conf.ElasticSearchIndex = os.Getenv("ELASTICSEARCH_INDEX")
	} else {
		conf.ElasticSearchIndex = "logstash-*"
	}
	log.Info("Using ElasticSearch Index %s.", conf.ElasticSearchIndex)

	appContext := server.AppContext{}
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
	appContext.ArchiveService = elasticsearch.NewArchiveService(elasticSearch)

	router := mux.NewRouter()

	router.HandleFunc("/eve2pcap", evebox.Eve2PcapHandler)
	router.HandleFunc("/api/version", VersionHandler)
	router.Handle("/api/config", ConfigHandler(appContext))

	router.Handle("/api/1/archive", server.Api(appContext,
		&server.ArchiveHandler{appContext}))

	setupElasticSearchProxy(router)
	evebox.SetupStatic(router, opts.DevServerUri)

	log.Printf("Listening on %s:%s", opts.Host, opts.Port)
	err = http.ListenAndServe(opts.Host + ":" + opts.Port, router)
	if err != nil {
		log.Fatal(err)
	}
}

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

package main

import (
	"encoding/json"
	"fmt"
	"log"
	"net/http"
	"net/http/httputil"
	"net/url"
	"os"

	"github.com/GeertJohan/go.rice"
	"github.com/jessevdk/go-flags"
)

var buildDate string
var buildVersion string
var buildRev string

const DEFAULT_ELASTICSEARCH_URI string = "http://localhost:9200"

var opts struct {
	// We don't provide a default for this one so we can easily
	// detect if its been set or not.
	ElasticSearchUri string `long:"elasticsearch" short:"e" description:"Elastic Search URI (default: http://localhost:9200)"`
	Port             string `long:"port" short:"p" default:"5636" description:"Port to bind to"`
	Host             string `long:"host" default:"0.0.0.0" description:"Host to bind to"`
	DevServerUri     string `long:"dev" description:"Frontend development server URI"`
	Version          bool   `long:"version" description:"Show version"`
	Config           string `long:"config" short:"c" description:"Configuration filename"`
}

var config = &Config{}

type VersionResponse struct {
	Version  string
	Revision string
	Date     string
}

func VersionHandler(w http.ResponseWriter, r *http.Request) {
	response := VersionResponse{
		buildVersion,
		buildRev,
		buildDate,
	}
	w.Header().Set("Content-Type", "application/json; charset=UTF-8")
	json.NewEncoder(w).Encode(response)
}

func ConfigHandler(w http.ResponseWriter, r *http.Request) {
	configJson, err := config.toJSON()
	if err != nil {
		// Return failure.
		log.Println("error: ", err)
		return
	}
	w.Header().Set("Content-Type", "application/json; charset=UTF-8")
	w.Write(configJson)
}

func setupElasticSearchProxy() {
	if opts.ElasticSearchUri == "" {
		if os.Getenv("ELASTICSEARCH_URL") != "" {
			opts.ElasticSearchUri = os.Getenv("ELASTICSEARCH_URL")
		} else {
			opts.ElasticSearchUri = DEFAULT_ELASTICSEARCH_URI
		}
	}
	log.Printf("Elastic Search URI: %v", opts.ElasticSearchUri)
	esProxy, err := NewElasticSearchProxy(opts.ElasticSearchUri,
		"/elasticsearch")
	if err != nil {
		log.Fatal(err)
	}
	http.Handle("/elasticsearch/", esProxy)
}

// Setup the handler for static files.
func setupStatic() {
	if len(opts.DevServerUri) > 0 {
		log.Printf("Proxying static files to development server %v.",
			opts.DevServerUri)
		devServerProxyUrl, err := url.Parse(opts.DevServerUri)
		if err != nil {
			log.Fatal(err)
		}
		devServerProxy :=
			httputil.NewSingleHostReverseProxy(devServerProxyUrl)
		http.Handle("/", devServerProxy)
	} else {
		public := http.FileServer(
			rice.MustFindBox("./public").HTTPBox())
		http.Handle("/", public)
	}
}

func main() {

	_, err := flags.Parse(&opts)
	if err != nil {
		// flags.Parse should have already presented an error message.
		os.Exit(1)
	}

	if opts.Version {
		fmt.Printf("EveBox Version %s (rev %s) [%s]\n",
			buildVersion, buildRev, buildDate)
		os.Exit(0)
	}

	// If no configuration was provided, see if evebox.yaml exists
	// in the current directory.
	if opts.Config == "" {
		_, err := os.Stat("./evebox.yaml")
		if err == nil {
			opts.Config = "./evebox.yaml"
		}
	}
	if opts.Config != "" {
		log.Printf("Loading configuration file %s.\n", opts.Config)
		config, err = LoadConfig(opts.Config)
		if err != nil {
			log.Fatal(err)
		}
	}

	http.HandleFunc("/eve2pcap", Eve2PcapHandler)
	http.HandleFunc("/api/version", VersionHandler)
	http.HandleFunc("/api/config", ConfigHandler)
	setupElasticSearchProxy()
	setupStatic()

	log.Printf("Listening on %s:%s", opts.Host, opts.Port)
	err = http.ListenAndServe(opts.Host+":"+opts.Port, nil)
	if err != nil {
		log.Fatal(err)
	}
}

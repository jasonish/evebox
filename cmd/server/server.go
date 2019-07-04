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
	"errors"
	"fmt"
	"github.com/jasonish/evebox/resources"

	"github.com/jasonish/evebox/appcontext"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/elasticsearch"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/evereader"
	"github.com/jasonish/evebox/exiter"
	"github.com/jasonish/evebox/geoip"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/postgres"
	"github.com/jasonish/evebox/rules"
	"github.com/jasonish/evebox/server"
	"github.com/jasonish/evebox/sqlite"
	"github.com/jasonish/evebox/sqlite/configdb"
	"github.com/jasonish/evebox/useragent"
	"github.com/spf13/pflag"
	"github.com/spf13/viper"
	"runtime"
	"strings"
	"time"
)

const DEFAULT_DATA_DIR = ""
const DEFAULT_ELASTICSEARCH_URL = "http://localhost:9200"
const DEFAULT_ELASTICSEARCH_INDEX = "logstash"

const HTTP_TLS_ENABLED_KEY = "http.tls.enabled"
const HTTP_TLS_CERT_KEY = "http.tls.certificate"
const HTTP_TLS_KEY_KEY = "http.tls.key"

var opts struct {
	Port               uint16
	Host               string
	Version            bool
	NoCheckCertificate bool
}

func VersionMain() {
	fmt.Printf("EveBox Version %s (rev %s)\n",
		core.BuildVersion, core.BuildRev)
}

func initViper() {
	viper.SetDefault("data-directory", DEFAULT_DATA_DIR)
	viper.BindEnv("data-directory", "EVEBOX_DATA_DIRECTORY")

	viper.SetDefault("http.reverse-proxy", false)
	viper.BindEnv("http.reverse-proxy", "EVEBOX_HTTP_REVERSE_PROXY")

	viper.SetDefault("http.request-logging", false)
	viper.BindEnv("http.request-logging", "EVEBOX_HTTP_REQUEST_LOGGING")

	viper.SetDefault("elasticsearch", DEFAULT_ELASTICSEARCH_URL)
	viper.SetDefault("index", DEFAULT_ELASTICSEARCH_INDEX)

	// Retention period in days.
	viper.SetDefault("database.retention-period", 0)
	viper.BindEnv("database.retention-period", "RETENTION_PERIOD")

	viper.BindEnv("input.bookmark-directory", "BOOKMARK_DIRECTORY")

	viper.SetDefault("authentication.required", false)
	viper.BindEnv("authentication.required",
		"EVEBOX_AUTHENTICATION_REQUIRED")

	viper.SetDefault("authentication.type", "username")
	viper.BindEnv("authentication.type",
		"EVEBOX_AUTHENTICATION_TYPE")

	viper.BindEnv("authentication.login-message",
		"EVEBOX_AUTHENTICATION_LOGIN_MESSAGE")

	viper.BindEnv("authentication.github.client-id", "GITHUB_CLIENT_ID")
	viper.BindEnv("authentication.github.client-secret", "GITHUB_CLIENT_SECRET")

	// Defaults for PostgreSQL database.
	viper.SetDefault("database.postgresql.managed", true)
	viper.BindEnv("database.postgresql.managed", "PGMANAGED")

	viper.SetDefault("database.postgresql.host", "localhost")
	viper.BindEnv("database.postgresql.host", "PGHOST")

	viper.SetDefault("database.postgresql.database", "evebox")
	viper.BindEnv("database.postgresql.database", "PGDATABASE")

	viper.SetDefault("database.postgresql.user", "evebox")
	viper.BindEnv("database.postgresql.user", "PGUSER")

	viper.SetDefault("database.postgresql.password", "")
	viper.BindEnv("database.postgresql.password", "PGPASSWORD")
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

func configure(config *appcontext.Config) {

	config.Http.TlsEnabled = viper.GetBool(HTTP_TLS_ENABLED_KEY)
	config.Http.TlsCertificate = viper.GetString(HTTP_TLS_CERT_KEY)
	config.Http.TlsKey = viper.GetString(HTTP_TLS_KEY_KEY)

	config.Http.ReverseProxy = viper.GetBool("http.reverse-proxy")
	config.Http.RequestLogging = viper.GetBool("http.request-logging")

	config.LetsEncryptHostname = viper.GetString("letsencrypt.hostname")

	config.Authentication.Required = viper.GetBool("authentication.required")
	if config.Authentication.Required {

		config.Authentication.Type =
			viper.GetString("authentication.type")
		config.Authentication.LoginMessage =
			viper.GetString("authentication.login-message")

		// GitHub.
		github := &config.Authentication.Github
		github.Enabled = viper.GetBool("authentication.github.enabled")
		if config.Authentication.Github.Enabled {
			github.ClientSecret =
				viper.GetString("authentication.github.client-secret")
			github.ClientID =
				viper.GetString("authentication.github.client-id")
			github.Callback = viper.GetString("authentication.github.callback")
		}
	}
}

func Main(args []string) {

	log.SetLevel(log.INFO)

	var configFilename string
	var err error
	verbose := false

	log.Info("This is EveBox Server version %v (rev: %v); os=%s, arch=%s",
		core.BuildVersion, core.BuildRev, runtime.GOOS, runtime.GOARCH)

	initViper()

	flagset := pflag.NewFlagSet("server", pflag.ExitOnError)

	// Prevent the "pflag: help requested" on help.
	pflag.ErrHelp = errors.New("")

	// Datastore type.
	flagset.String("datastore", "elasticsearch", "Datastore to use")
	viper.BindPFlag("database.type", flagset.Lookup("datastore"))
	viper.BindEnv("database.type", "DATABASE_TYPE")

	flagset.StringP("elasticsearch", "e", DEFAULT_ELASTICSEARCH_URL,
		"Elastic Search URI (default: http://localhost:9200")
	viper.BindPFlag("database.elasticsearch.url", flagset.Lookup("elasticsearch"))
	viper.BindEnv("database.elasticsearch.url", "ELASTICSEARCH_URL")

	flagset.StringP("index", "i", DEFAULT_ELASTICSEARCH_INDEX,
		"Elastic Search Index")
	viper.BindPFlag("database.elasticsearch.index", flagset.Lookup("index"))
	viper.BindEnv("index", "ELASTICSEARCH_INDEX")

	flagset.StringP("elasticsearch-template", "", "", "Elastic Search template name")
	viper.BindPFlag("database.elasticsearch.template", flagset.Lookup("elasticsearch-template"))
	viper.BindEnv("elasticsearch-template", "ELASTICSEARCH_TEMPLATE")

	viper.BindEnv("database.elasticsearch.username", "ELASTICSEARCH_USERNAME")
	viper.BindEnv("database.elasticsearch.password", "ELASTICSEARCH_PASSWORD")

	flagset.Bool("elasticsearch-force-template", false,
		"Force install/overwrite of Elasticsearch template")
	viper.BindPFlag("database.elasticsearch.force-template",
		flagset.Lookup("elasticsearch-force-template"))
	viper.BindEnv("database.elasticsearch.force-template",
		"ELASTICSEARCH_FORCE_TEMPLATE")

	// Elastic Search keyword. This is purposely not bound to viper as viper
	// fails to tell us if it was set or not.
	flagset.String("elasticsearch-keyword", "", "Elastic Search keyword")

	flagset.BoolVarP(&verbose, "verbose", "v", false, "Verbose (debug logging)")

	flagset.Uint16VarP(&opts.Port, "port", "p", server.DEFAULT_PORT, "Port to bind to")
	flagset.StringVarP(&opts.Host, "host", "", "0.0.0.0", "Host to bind to")
	flagset.BoolVarP(&opts.Version, "version", "", false, "Show version")
	flagset.StringVarP(&configFilename, "config", "c", "", "Configuration filename")
	flagset.BoolVarP(&opts.NoCheckCertificate, "no-check-certificate", "k", false, "Disable certificate check for Elastic Search")

	flagset.StringP("data-directory", "D", DEFAULT_DATA_DIR, "Data directory")
	viper.BindPFlag("data-directory", flagset.Lookup("data-directory"))

	flagset.Bool("tls", false, "Enable TLS")
	viper.BindPFlag(HTTP_TLS_ENABLED_KEY, flagset.Lookup("tls"))
	viper.BindEnv(HTTP_TLS_ENABLED_KEY, "EVEBOX_TLS_ENABLED")

	flagset.String("tls-cert", "", "TLS certificate filename")
	viper.BindPFlag(HTTP_TLS_CERT_KEY, flagset.Lookup("tls-cert"))
	viper.BindEnv(HTTP_TLS_CERT_KEY, "EVEBOX_TLS_CERT")

	flagset.String("tls-key", "", "TLS key filename")
	viper.BindPFlag(HTTP_TLS_KEY_KEY, flagset.Lookup("tls-key"))
	viper.BindEnv(HTTP_TLS_KEY_KEY, "EVEBOX_TLS_KEY")

	flagset.String("letsencrypt", "", "Letsencrypt hostname")
	viper.BindPFlag("letsencrypt.hostname", flagset.Lookup("letsencrypt"))

	var input string
	flagset.StringVar(&input, "input", "", "Input eve-log file (optional)")
	inputStart := flagset.Bool("input-start", false, "Read start of input file (if no bookmark)")

	flagset.Parse(args[0:])

	if opts.Version {
		VersionMain()
		return
	}

	if verbose {
		log.SetLevel(log.DEBUG)
	}

	// Self test.
	_, err = resources.GetEmbeddedAsset("./public/index.html")
	if err != nil {
		log.Error("Self test: no embedded index.html found.")
	} else {
		log.Info("Self test: found embedded index.html.")
	}

	if configFilename != "" {
		viper.SetConfigFile(configFilename)
		if strings.HasSuffix(configFilename, ".yaml.example") {
			viper.SetConfigType("yaml")
		}
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
	configure(&appContext.Config)
	appContext.GeoIpService = geoip.NewGeoIpService()

	datadir := viper.GetString("data-directory")
	if datadir == "" {
		appContext.ConfigDB, err = configdb.NewConfigDB(":memory:")
	} else {
		appContext.ConfigDB, err = configdb.NewConfigDB(datadir)
	}
	if err != nil {
		log.Fatal("Failed to initialize configuration database: %v", err)
	}

	// Authentication is not possible with an in-memory configuration
	// database.
	if appContext.ConfigDB.InMemory {
		required := viper.GetBool("authentication.required")
		authType := viper.GetString("authentication.type")
		if required && authType == "usernamepassword" {
			log.Fatal("Authentication requires a data-directory.")
		}
	}

	// Not sure about doing this with an in-memory store right now.
	appContext.Userstore = configdb.NewUserStore(appContext.ConfigDB.DB)

	switch viper.GetString("database.type") {
	case "elasticsearch":
		log.Info("Configuring ElasticSearch datastore")
		log.Info("Using ElasticSearch URL %s",
			viper.GetString("database.elasticsearch.url"))
		log.Info("Using ElasticSearch Index %s.",
			viper.GetString("database.elasticsearch.index"))

		config := elasticsearch.Config{
			BaseURL:          viper.GetString("database.elasticsearch.url"),
			DisableCertCheck: opts.NoCheckCertificate,
			Username:         viper.GetString("database.elasticsearch.username"),
			Password:         viper.GetString("database.elasticsearch.password"),
			Index:            viper.GetString("database.elasticsearch.index"),
			Template:         viper.GetString("database.elasticsearch.template"),
			ForceTemplate:    viper.GetBool("database.elasticsearch.force-template"),
		}

		// Configuration provided keyword suffix?
		isSet, keyword := getElasticSearchKeyword(flagset)
		if isSet {
			config.KeywordSuffix = keyword
			if keyword == "" {
				config.NoKeywordSuffix = true
			}
		}

		elasticSearch := elasticsearch.New(config)

		for {
			ping, err := elasticSearch.Ping()
			if err != nil {
				log.Error("Failed to ping Elastic Search, delaying startup: %v", err)
				time.Sleep(3 * time.Second)
			} else {
				log.Info("Connected to Elastic Search (version: %s)",
					ping.Version.Number)
				major, _ := ping.ParseVersion()
				if major < 5 {
					log.Fatalf("Elastic Search versions less than 5 are not supported.")
				}
				break
			}
		}

		// May want to loop here until this has succeeded...
		if err := elasticSearch.ConfigureIndex(); err != nil {
			log.Error("Failed to configure Elastic Search index, EveBox may not function properly: %v", err)
		}

		appContext.ElasticSearch = elasticSearch
		appContext.ReportService = elasticsearch.NewReportService(elasticSearch)
		appContext.DataStore, err = elasticsearch.NewDataStore(elasticSearch)
		if err != nil {
			log.Fatal(err)
		}
		appContext.SetFeature(core.FEATURE_REPORTING)
		appContext.SetFeature(core.FEATURE_COMMENTS)
	case "sqlite":
		// Requires data directory.
		if viper.GetString("data-directory") == "" {
			log.Fatalf("SQLite datastore requires a data-directory")
		}
		if err := sqlite.InitSqlite(&appContext); err != nil {
			log.Fatal(err)
		}
	case "postgresql":

		var pgConfig postgres.PgConfig

		dataDirectory := viper.GetString("data-directory")

		managed := viper.GetBool("database.postgresql.managed")
		if !managed {
			//log.Fatal("Unmanaged PostgreSQL not yet supported.")
			pgConfig = postgres.PgConfig{
				User:     viper.GetString("database.postgresql.user"),
				Password: viper.GetString("database.postgresql.password"),
				Host:     viper.GetString("database.postgresql.host"),
				Database: viper.GetString("database.postgresql.database"),
			}
		} else {
			// Requires data directory.
			if dataDirectory == "" {
				log.Fatalf("Managed PostgreSQL datastore requires a data-directory")
			}

			manager, err := postgres.ConfigureManaged(dataDirectory)
			if err != nil {
				log.Fatal(err)
			}
			manager.Start()
			exiter.AtExit(manager.StopFast)

			pgConfig, err = postgres.ManagedConfig(dataDirectory)
			if err != nil {
				log.Fatal(err)
			}
		}

		// Try a few times to connect to the database sleeping for a bit on
		// failure. Useful when using something like Docker compose where the
		// database might not be ready on the first connection attempt.
		var pg *postgres.PgDB
		tryCount := 5
		for tryCount > 0 {
			tryCount--
			pg, err = postgres.NewPgDatabase(pgConfig)
			if err != nil {
				if tryCount == 0 {
					log.Fatal(err)
				}
				log.Warning("Failed to connect to database, will try again: %v", err)
				time.Sleep(1 * time.Second)
			} else {
				break
			}
		}

		pgMigrator := postgres.NewSqlMigrator(pg, "postgres")
		pgMigrator.Migrate()

		appContext.DataStore = postgres.NewPgDatastore(pg)

		appContext.SetFeature(core.FEATURE_COMMENTS)
	default:
		log.Fatal("unsupported datastore: ",
			viper.GetString("database.type"))
	}

	initInternalEveReader(&appContext, *inputStart)

	httpServer := server.NewServer(appContext)
	err = httpServer.Start(opts.Host, opts.Port)
	if err != nil {
		log.Fatal(err)
	}

	exiter.Exit(0)
}

func initInternalEveReader(appContext *appcontext.AppContext, inputStart bool) {
	enabled := viper.GetBool("input.enabled")
	if !enabled {
		return
	}
	log.Info("Configuring internal eve log reader")
	filename := viper.GetString("input.filename")
	bookmarkDirectory := viper.GetString("input.bookmark-directory")

	eventSink := appContext.DataStore.GetEveEventSink()
	if eventSink == nil {
		log.Fatal("Selected datastore does not provide an event sink.")
	}

	eveFileProcessor := &evereader.EveFileProcessor{
		Filename:          filename,
		BookmarkDirectory: bookmarkDirectory,
		Sink:              eventSink,
		End:               !inputStart,
	}

	eveFileProcessor.AddFilter(&eve.TagsFilter{})
	eveFileProcessor.AddFilter(eve.NewGeoipFilter(appContext.GeoIpService))
	eveFileProcessor.AddFilter(&useragent.EveUserAgentFilter{})

	inputRules := viper.GetStringSlice("input.rules")
	if inputRules != nil {
		ruleMap := rules.NewRuleMap(inputRules)
		eveFileProcessor.AddFilter(ruleMap)
	}

	for field, value := range viper.GetStringMap("input.custom-fields") {
		eveFileProcessor.AddCustomField(field, value)
	}

	eveFileProcessor.Start()
}

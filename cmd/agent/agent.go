/* Copyright (c) 2017 Jason Ish
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

package agent

import (
	"github.com/jasonish/evebox/agent"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/evereader"
	"github.com/jasonish/evebox/log"
	"github.com/spf13/pflag"
	"github.com/spf13/viper"
	"os"
	"os/signal"
	"strings"
	"syscall"
)

var flagset *pflag.FlagSet

func initViper() {
	viper.SetDefault("disable-certificate-check", false)

	viper.BindEnv("bookmark-directory", "BOOKMARK_DIRECTORY")

	viper.BindEnv("server.url", "EVEBOX_AGENT_SERVER")
	viper.BindEnv("server.username", "EVEBOX_AGENT_USERNAME")
	viper.BindEnv("server.password", "EVEBOX_AGENT_PASSWORD")
}

func configure(args []string) {
	var configFilename string

	flagset = pflag.NewFlagSet("agent", 0)

	flagset.StringVarP(&configFilename, "config", "c", "", "Configuration file")
	viper.BindPFlag("config", flagset.Lookup("config"))

	flagset.BoolP("verbose", "v", false, "Be more verbose")
	viper.BindPFlag("verbose", flagset.Lookup("verbose"))

	flagset.Bool("stdout", false, "Print events to stdout")
	viper.BindPFlag("stdout", flagset.Lookup("stdout"))

	flagset.String("server", "", "EveBox server URL")
	viper.BindPFlag("server", flagset.Lookup("server"))

	if err := flagset.Parse(args); err != nil {
		if err == pflag.ErrHelp {
			os.Exit(0)
		}
		os.Exit(1)
	}

	if configFilename != "" {
		log.Info("Using configuration file %s", configFilename)
		viper.SetConfigFile(configFilename)
		if strings.Index(configFilename, "yaml") > -1 {
			viper.SetConfigType("yaml")
		}
	} else {
		viper.SetConfigName("agent")
		viper.SetConfigType("yaml")
		viper.AddConfigPath(".")
		viper.AddConfigPath("/etc/evebox")
	}

	err := viper.ReadInConfig()
	if err != nil {
		log.Fatal(err)
	}

	verbose := viper.GetBool("verbose")
	if verbose {
		log.SetLevel(log.DEBUG)
	}
}

func Main(args []string) {

	initViper()
	configure(args)

	var serverUrl string
	var serverUsername string
	var serverPassword string

	serverNode := viper.Get("server")
	switch serverNode.(type) {
	case string:
		serverUrl = viper.GetString("server")
	case map[string]interface{}:
		serverUrl = viper.GetString("server.url")
		serverUsername = viper.GetString("server.username")
		serverPassword = viper.GetString("server.password")
	}

	if serverUrl == "" {
		log.Fatal("error: no server url provided")
	}

	client := agent.NewClient()
	client.SetBaseUrl(serverUrl)

	log.Info("Username: %s; password: %s", serverUsername, serverPassword)

	if serverUsername != "" || serverPassword != "" {
		client.SetUsernamePassword(serverUsername, serverPassword)
	}

	version, err := client.GetVersion()
	if err != nil {
		log.Error("Failed to query server for version, will continue: %v", err)
	} else {
		log.Info("Connected to EveBox version %s", version.Get("version"))
	}

	if !viper.InConfig("input") {
		log.Fatal("No input configured.")
	}
	path := viper.GetString("input.filename")

	eveFileProcessor := evereader.EveFileProcessor{
		Filename:          path,
		BookmarkDirectory: viper.GetString("bookmark-directory"),
		Sink:              agent.NewEventChannel(client),
	}
	eveFileProcessor.AddFilter(&eve.TagsFilter{})
	for field, value := range viper.GetStringMap("input.custom-fields") {
		eveFileProcessor.AddCustomField(field, value)
	}
	eveFileProcessor.Start()

	sigchan := make(chan os.Signal)
	signal.Notify(sigchan, os.Interrupt, syscall.SIGTERM)
	for sig := range sigchan {
		log.Info("Got signal %d, stopping.", sig)
		eveFileProcessor.Stop()
		break
	}
}

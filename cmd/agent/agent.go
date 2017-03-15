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
	"github.com/jasonish/evebox/log"
	"github.com/spf13/pflag"
	"github.com/spf13/viper"
	"os"
	"os/signal"
	"syscall"
)

var flagset *pflag.FlagSet

func setDefaults() {
	viper.SetDefault("disable-certificate-check", false)
}

func configure(args []string) {
	var configFilename string

	flagset = pflag.NewFlagSet("import", pflag.ExitOnError)

	flagset.StringVarP(&configFilename, "config", "c", "", "Configuration file")
	viper.BindPFlag("config", flagset.Lookup("config"))

	flagset.BoolP("verbose", "v", false, "Be more verbose")
	viper.BindPFlag("verbose", flagset.Lookup("verbose"))

	flagset.Bool("stdout", false, "Print events to stdout")
	viper.BindPFlag("stdout", flagset.Lookup("stdout"))

	flagset.String("server", "", "EveBox server URL")
	viper.BindPFlag("server", flagset.Lookup("server"))

	flagset.Parse(args)

	if configFilename != "" {
		log.Info("Using configuration file %s", configFilename)
		viper.SetConfigFile(configFilename)
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

	setDefaults()
	configure(args)

	if viper.GetString("server") == "" {
		log.Fatal("error: no server url provided")
	}

	client := agent.NewClient()
	client.SetBaseUrl(viper.GetString("server"))
	version, err := client.GetVersion()
	if err != nil {
		log.Error("Failed to query server for version, will continue: %v", err)
	} else {
		log.Info("Connected to EveBox version %s", version.Get("version"))
	}

	if !viper.InConfig("input") {
		log.Fatal("No input configured.")
	}
	path := viper.GetString("input.path")

	readerRunner := agent.ReaderLoop{
		Path:              path,
		BookmarkDirectory: viper.GetString("bookmark-directory"),
		EventSink:         agent.NewEventChannel(client),
		CustomFields:      viper.GetStringMap("input.custom-fields"),
	}
	go readerRunner.Run()

	sigchan := make(chan os.Signal)
	signal.Notify(sigchan, os.Interrupt, syscall.SIGTERM)
	for sig := range sigchan {
		log.Info("Got signal %d, stopping.", sig)
		readerRunner.Stop()
		break
	}
}

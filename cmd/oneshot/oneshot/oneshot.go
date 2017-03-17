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

package oneshot

import (
	"fmt"

	"github.com/jasonish/evebox/agent"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/geoip"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/server"
	"github.com/jasonish/evebox/sqlite"
	"github.com/spf13/pflag"
	"github.com/spf13/viper"
	"io/ioutil"
	"os"
	"os/exec"
	"os/signal"
	"path/filepath"
	"sync"
)

var opts struct {
	Port             string
	Host             string
	Version          bool
	DatabaseFilename string
	InMemory         bool
}

func VersionMain() {
	fmt.Printf("EveBox Version %s (rev %s)\n",
		core.BuildVersion, core.BuildRev)
}

func setDefaults() {
	viper.SetDefault("database.retention-period", 0)
}

func Main(args []string) {

	log.SetLevel(log.DEBUG)

	var err error

	log.Info("This is EveBox Server version %v (rev: %v)", core.BuildVersion, core.BuildRev)

	setDefaults()

	flagset := pflag.NewFlagSet("server", pflag.ExitOnError)

	flagset.StringVarP(&opts.Port, "port", "p", "5636", "Port to bind to")
	flagset.StringVarP(&opts.Host, "host", "", "0.0.0.0", "Host to bind to")
	flagset.BoolVarP(&opts.Version, "version", "", false, "Show version")

	flagset.StringVarP(&opts.DatabaseFilename, "database-filename", "D", "", "Database filename")
	flagset.BoolVar(&opts.InMemory, "in-memory", false, "Use in-memory database")

	flagset.Parse(args[0:])

	if opts.Version {
		VersionMain()
		return
	}

	appContext := server.AppContext{}
	appContext.GeoIpService = geoip.NewGeoIpService()

	if opts.InMemory {
		log.Info("Using in-memory database")
		viper.Set("database.sqlite.filename", ":memory:")
	} else {
		if opts.DatabaseFilename == "" {
			tmp, err := ioutil.TempFile(".", "evebox-oneshot")
			if err != nil {
				log.Fatal(err)
			}
			log.Info("Using temporary file %s", tmp.Name())
			viper.Set("database.sqlite.filename", tmp.Name())
			defer func() {
				filenames, err := filepath.Glob("./" + tmp.Name() + "*")
				if err != nil {
					log.Error("Failed to cleanup temporary files.")
				} else {
					for _, filename := range filenames {
						log.Info("Deleting %s.", filename)
						os.Remove(filename)
					}
				}
			}()
		} else {
			log.Info("Using database file %s.", opts.DatabaseFilename)
			viper.Set("database.sqlite.filename", opts.DatabaseFilename)
			defer func() {
				log.Info("Database file %s will not be removed.", opts.DatabaseFilename)
			}()
		}
	}

	if err := sqlite.InitSqlite(&appContext); err != nil {
		log.Fatal(err)
	}

	wg := sync.WaitGroup{}
	wg.Add(1)
	go func() {
		for _, filename := range flagset.Args() {
			log.Println("Importing", filename)
			readerLoop := agent.ReaderLoop{
				Path:               filename,
				DisableBookmarking: true,
				EventSink:          appContext.DataStore.GetEveEventConsumer(),
				Oneshot:            true,
			}
			readerLoop.Run()
		}
		log.Println("Import done.")
		wg.Done()
	}()

	sigchan := make(chan os.Signal)
	signal.Notify(sigchan, os.Interrupt)
	for sig := range sigchan {
		log.Info("Caught signal %d.", sig)
		return
	}
	wg.Wait()

	go func() {
		httpServer := server.NewServer(appContext)
		err = httpServer.Start(opts.Host + ":" + opts.Port)
		if err != nil {
			log.Fatal(err)
		}
	}()

	log.Println("Server is running.")

	c := exec.Command("xdg-open", "http://localhost:"+opts.Port)
	c.Run()

	signal.Notify(sigchan, os.Interrupt)
	<-sigchan
	log.Info("Cleaning up and exiting...")
}

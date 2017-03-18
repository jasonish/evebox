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
	"strconv"
	"time"
)

const DEFAULT_PORT = 5636

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

	flagset.StringVarP(&opts.Port, "port", "p", "", "Port to bind to")
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

	sigchan := make(chan os.Signal)
	waitchan := make(chan int)
	signal.Notify(sigchan, os.Interrupt)

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
		waitchan <- 0
	}()

	select {
	case <-sigchan:
		return
	case <-waitchan:
		break
	}

	portChan := make(chan int64, 0xffff)

	log.Info("Starting server.")
	go func() {
		port := int64(DEFAULT_PORT)
		if opts.Port != "" {
			port, err = strconv.ParseInt(opts.Port, 10, 16)
			if err != nil {
				log.Warning("Failed to parse port \"%s\", will use default of %d", DEFAULT_PORT)
				port = DEFAULT_PORT
			}
		}
		httpServer := server.NewServer(appContext)
		for {
			portChan <- port
			err = httpServer.Start(fmt.Sprintf("%s:%d", opts.Host, port))
			if err != nil {
				log.Warning("Failed to bind to port %d: %v", port, err)
				port++
				if port > 0xffff {
					log.Fatal("Exhausted all ports, exiting.")
					break
				}
			} else {
				break
			}
		}
	}()

	// What a hack to make sure we successfully bound to a port, and to
	// get that port.
	var port int64
	var done bool
	waitTime := 100
	for {
		if done {
			break
		}
		select {
		case port = <-portChan:
			waitTime = 100
		default:
			if waitTime > 0 {
				time.Sleep(time.Duration(waitTime) * time.Millisecond)
				waitTime = 0
			} else {
				done = true
			}
		}
	}

	log.Info("Bound to port %d", port)
	log.Println("Server is running.")
	url := fmt.Sprintf("http://localhost:%d", port)
	c := exec.Command("xdg-open", url)
	c.Run()

	fmt.Printf("\n** Press CTRL-C to exit and cleanup.. ** \n\n")

	<-sigchan
	log.Info("Cleaning up and exiting...")
}

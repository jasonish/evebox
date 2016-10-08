/* Copyright (c) 2016 Jason Ish
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

package esimport

import (
	"fmt"
	"github.com/jasonish/evebox/elasticsearch"
	"github.com/jasonish/evebox/evereader"
	flag "github.com/spf13/pflag"
	"os"
	"io"
	"time"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/geoip"
	"encoding/json"
	"net"
)

var RFC1918_Netstrings = []string{
	"10.0.0.0/8",
	"127.16.0.0/12",
	"192.168.0.0/16",
}

var RFC1918_IPNets []*net.IPNet

func init() {
	for _, network := range (RFC1918_Netstrings) {
		_, ipnet, err := net.ParseCIDR(network)
		if err == nil {
			RFC1918_IPNets = append(RFC1918_IPNets, ipnet)
		}
	}
}

var flagset *flag.FlagSet

func usage() {
	usage := `Usage: evebox import [options] /path/to/eve.json

Options:
`
	fmt.Fprint(os.Stderr, usage)
	flagset.PrintDefaults()
}

func IsRFC1918(addr string) bool {
	ip := net.ParseIP(addr)
	for _, ipnet := range (RFC1918_IPNets) {
		if ipnet.Contains(ip) {
			return true
		}
	}
	return false
}

func Main(args []string) {

	var elasticSearchUri string
	var oneshot bool
	var index string
	var verbose bool
	var end bool
	var batchSize uint64
	var useBookmark bool
	var bookmarkPath string
	var stdout bool
	var nogeoip bool
	var geoIpDatabase string
	var noCheckCertificate bool
	var usernamePassword string

	flagset = flag.NewFlagSet("import", flag.ExitOnError)
	flagset.Usage = usage
	flagset.StringVarP(&elasticSearchUri, "elasticsearch", "e", "", "Elastic Search URL")
	flagset.BoolVar(&oneshot, "oneshot", false, "One shot mode (exit on EOF)")
	flagset.StringVar(&index, "index", "evebox", "Elastic Search index prefix")
	flagset.BoolVarP(&verbose, "verbose", "v", false, "Verbose output")
	flagset.BoolVar(&end, "end", false, "Start at end of file")
	flagset.Uint64Var(&batchSize, "batch-size", 1000, "Batch import size")
	flagset.BoolVar(&useBookmark, "bookmark", false, "Bookmark location")
	flagset.StringVar(&bookmarkPath, "bookmark-path", "", "Path to bookmark file")
	flagset.BoolVar(&nogeoip, "no-geoip", false, "Disable GeoIP lookups")
	flagset.BoolVar(&stdout, "stdout", false, "Print events to stdout")
	flagset.StringVar(&geoIpDatabase, "geoip-database", "", "Path to GeoIP (v2) database file")
	flagset.BoolVarP(&noCheckCertificate, "no-check-certificate", "k", false, "Disable certificate check")
	flagset.StringVarP(&usernamePassword, "user", "u", "", "Username:password")

	flagset.Parse(args[1:])

	if verbose {
		log.SetLevel(log.DEBUG)
	}

	if batchSize < 1 {
		log.Fatal("Batch size must be greater than 0")
	}

	if elasticSearchUri == "" {
		log.Error("error: --elasticsearch is a required parameter")
		usage()
		os.Exit(1)
	}

	if len(flagset.Args()) == 0 {
		log.Fatal("error: no input file provided")
	}

	if useBookmark && bookmarkPath == "" {
		bookmarkPath = fmt.Sprintf("%s.bookmark", flagset.Args()[0])
		log.Info("Using bookmark file %s", bookmarkPath)
	}

	es := elasticsearch.New(elasticSearchUri)
	es.DisableCertCheck = noCheckCertificate
	if usernamePassword != "" {
		if err := es.SetUsernamePassword(usernamePassword); err != nil {
			log.Fatal("Failed to set username:password: %v", err)
		}
	}
	response, err := es.Ping()
	if err != nil {
		log.Fatal("error: failed to ping Elastic Search:", err)
	}
	log.Info("Connected to Elastic Search v%s (cluster:%s; name: %s)",
		response.Version.Number, response.ClusterName, response.Name)

	// Check if the template exists.
	templateExists, err := es.CheckTemplate(index)
	if !templateExists {
		log.Info("Template %s does not exist, creating...", index)
		err = es.LoadTemplate(index)
		if err != nil {
			log.Fatal("Failed to create template:", err)
		}
	} else {
		log.Info("Template %s exists, will not create.", index)
	}

	var geoipdb *geoip.GeoIpDb

	if !nogeoip {
		geoipdb, err = geoip.NewGeoIpDb(geoIpDatabase)
		if err != nil {
			log.Notice("Failed to load GeoIP database: %v", err)
		} else {
			log.Info("Using GeoIP database %s, %s", geoipdb.Type(), geoipdb.BuildDate())
		}
	}

	inputFiles := flagset.Args()

	indexer := elasticsearch.NewIndexer(es, noCheckCertificate)
	indexer.IndexPrefix = index

	reader, err := evereader.New(inputFiles[0])
	if err != nil {
		log.Fatal(err)
	}

	// Initialize bookmarking...
	var bookmarker *evereader.Bookmarker = nil
	if useBookmark {
		bookmarker = &evereader.Bookmarker{
			Filename: bookmarkPath,
			Reader: reader,
		}
		err := bookmarker.Init(end)
		if err != nil {
			log.Fatal(err)
		}
	} else if end {
		log.Info("Jumping to end of file.")
		err := reader.SkipToEnd()
		if err != nil {
			log.Fatal(err)
		}
	}

	count := uint64(0)
	lastStatTs := time.Now()
	lastStatCount := uint64(0)
	startTime := time.Now()

	// Number of EOFs in last stat interval.
	eofs := uint64(0)

	go func() {
		err := indexer.Run()
		if err != nil {
			log.Fatal("Elastic Search indexer connection unexpectedly closed:", err)
		} else {
			log.Debug("Indexer exited without issue.")
		}
	}()

	for {
		eof := false
		event, err := reader.Next()
		if err != nil {
			if err == io.EOF {
				eof = true
				eofs++
			} else {
				log.Fatal(err)
			}
		}

		if event != nil {

			if geoipdb != nil {
				srcip, ok := event["src_ip"].(string)
				if ok && !IsRFC1918(srcip) {
					gip, err := geoipdb.LookupString(srcip)
					if err != nil {
						log.Debug("Failed to lookup geoip for %s", srcip)
					}

					// Need at least a continent code.
					if gip.ContinentCode != "" {
						event["geoip"] = gip
					}
				}
				if event["geoip"] == nil {
					destip, ok := event["dest_ip"].(string)
					if ok && !IsRFC1918(destip) {
						gip, err := geoipdb.LookupString(destip)
						if err != nil {
							log.Debug("Failed to lookup geoip for %s", destip)
						}
						// Need at least a continent code.
						if gip.ContinentCode != "" {
							event["geoip"] = gip
						}
					}
				}
			}

			if stdout {
				asJson, err := json.Marshal(event)
				if err != nil {
					log.Error("Failed to print event as json: %v", err)
				} else {
					fmt.Println(string(asJson))
				}
			}

			indexer.IndexRawEvent(event)
			count++
		}

		if eof || (count > 0 && count % batchSize == 0) {
			var bookmark *evereader.Bookmark = nil

			if useBookmark {
				bookmark = bookmarker.GetBookmark()
			}

			response, err := indexer.FlushConnection()
			if err != nil {
				log.Fatal(err)
			}
			if response != nil {
				log.Debug("Indexed %d events {errors=%v}", len(response.Items),
					response.Errors)
			}

			if useBookmark {
				bookmarker.WriteBookmark(bookmark)
			}
		}

		now := time.Now()
		if now.Sub(lastStatTs).Seconds() > 1 && now.Second() == 0 {

			// Calculate the lag in bytes, that is the number of bytes behind
			// the end of file we are.
			lag, err := GetLag(reader)
			if err != nil {
				log.Error("Failed to calculate lag: %v", err)
			}

			log.Info("Total: %d; Last minute: %d; Avg: %.2f/s, EOFs: %d; Lag (bytes): %d",
				count,
				count - lastStatCount,
				float64(count - lastStatCount) / (now.Sub(lastStatTs).Seconds()),
				eofs,
				lag)
			lastStatTs = now
			lastStatCount = count
			eofs = 0
		}

		if eof {
			if oneshot {
				break;
			} else {
				time.Sleep(1 * time.Second)
			}
		}
	}

	totalTime := time.Since(startTime)

	if oneshot {
		log.Info("Indexed %d events: time=%.2fs; avg=%d/s", count, totalTime.Seconds(),
			uint64(float64(count) / totalTime.Seconds()))
	}
}

func GetLag(reader *evereader.EveReader) (int64, error) {
	fileSize, err := reader.FileSize()
	if err != nil {
		return 0, err
	}
	fileOffset, err := reader.FileOffset()
	if err != nil {
		return 0, err
	}
	return fileSize - fileOffset, nil
}
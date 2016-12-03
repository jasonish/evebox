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

package evereader

import (
	"encoding/json"
	"fmt"
	"github.com/jasonish/evebox/evereader"
	"github.com/jasonish/evebox/log"
	flag "github.com/spf13/pflag"
	"io"
	"os"
	"time"
)

func Main(args []string) {

	var end bool
	var oneshot bool
	var useBookmark bool
	var bookmarkPath string
	var verbose bool

	flagset := flag.NewFlagSet("evereader", flag.ExitOnError)
	flagset.BoolVar(&end, "end", false, "Start at end of file")
	flagset.BoolVar(&oneshot, "oneshot", false, "One shot mode (exit on EOF)")
	flagset.BoolVar(&useBookmark, "bookmark", false, "Bookmark location")
	flagset.StringVar(&bookmarkPath, "bookmark-path", "", "Path to bookmark file")
	flagset.BoolVarP(&verbose, "verbose", "v", false, "Verbose output")
	flagset.Parse(args[1:])

	if verbose {
		log.SetLevel(log.DEBUG)
	}

	if len(flagset.Args()) == 0 {
		log.Fatal("No input files provided.")
	} else if len(flagset.Args()) > 1 {
		log.Fatal("Only one input file allowed.")
	}
	inputFilename := flagset.Args()[0]

	// If useBookmark but no path, set a default.
	if useBookmark && bookmarkPath == "" {
		bookmarkPath = fmt.Sprintf("%s.bookmark", inputFilename)
	}

	encoder := json.NewEncoder(os.Stdout)

	reader, err := evereader.New(flagset.Args()[0])
	if err != nil {
		log.Fatal(err)
	}

	// Initialize bookmark.
	var bookmarker *evereader.Bookmarker = nil
	if useBookmark {
		bookmarker = &evereader.Bookmarker{
			Filename: bookmarkPath,
			Reader:   reader,
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

	for {
		eof := false
		event, err := reader.Next()
		if err != nil {
			if err == io.EOF {
				if oneshot {
					break
				}
				eof = true
			} else {
				log.Fatal(err)
			}
		}

		if eof {
			time.Sleep(1 * time.Second)
		} else {
			encoder.Encode(event)

			if useBookmark {
				bookmark := bookmarker.GetBookmark()
				bookmarker.WriteBookmark(bookmark)
			}
		}
	}
}

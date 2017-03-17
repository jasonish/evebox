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
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/evereader"
	"github.com/jasonish/evebox/log"
	"io"
	"time"
)

const BATCH_SIZE = 1000

type ReaderLoop struct {
	Path               string
	BookmarkDirectory  string
	EventSink          core.EveEventSink
	CustomFields       map[string]interface{}
	DisableBookmarking bool
	Oneshot            bool

	bookmarker *evereader.Bookmarker
	stop       bool
}

func (r *ReaderLoop) addCustomFields(event eve.EveEvent) {
	for key := range r.CustomFields {
		event[key] = r.CustomFields[key]
	}
}

func (r *ReaderLoop) readFile(reader *evereader.EveReader) {

	count := 0
	lastFlushCount := 0
	lastStatTime := time.Now()
	lastStatCount := 0
	eofs := 0

	tagsFilter := eve.TagsFilter{}

	for {
		if r.stop {
			break
		}

		eof := false

		event, err := reader.Next()
		if err != nil {
			if err == io.EOF {
				eof = true
				eofs++
			} else {
				log.Error("Failed to read event: %v", err)
				continue
			}
		}

		if event != nil {
			tagsFilter.Filter(event)
			r.addCustomFields(event)

			err := r.EventSink.Submit(event)
			if err != nil {
				log.Error("Failed to submit event: %v", err)
				continue
			}
			count++
		}

		if eof || count > 0 && count%BATCH_SIZE == 0 {
			flushCount := count - lastFlushCount
			lastFlushCount = count

			if flushCount > 0 {

				var bookmark *evereader.Bookmark

				if r.bookmarker != nil {
					bookmark = r.bookmarker.GetBookmark()
				}

				for {
					_, err := r.EventSink.Commit()
					if err != nil {
						log.Error("Failed to commit events, will try again: %v", err)
						time.Sleep(1 * time.Second)
					} else {
						break
					}
				}

				log.Debug("Committed %d events", flushCount)
				if r.bookmarker != nil {
					r.bookmarker.WriteBookmark(bookmark)
				}

			}
			lastFlushCount = count
		}

		now := time.Now()

		if now.Sub(lastStatTime).Seconds() > 1 && now.Second() == 0 {
			lag, _ := reader.Lag()
			log.Info("Total: %d; last minute: %d; eofs: %d; lag: %d",
				count, count-lastStatCount, eofs, lag)
			lastStatCount = count
			eofs = 0
			lastStatTime = now
		}

		if eof {
			if r.Oneshot {
				break
			}
			time.Sleep(1 * time.Second)
		}
	}

	log.Debug("Returning from reading file %s", r.Path)
}

func (r *ReaderLoop) Run() {

	for {
		reader, err := evereader.New(r.Path)
		if err != nil {
			log.Warning("%v", err)
			time.Sleep(1 * time.Second)
			continue
		}

		if !r.DisableBookmarking {
			r.bookmarker = evereader.ConfigureBookmarker(r.Path, r.BookmarkDirectory, reader)
			if err := r.bookmarker.Init(false); err != nil {
				log.Fatalf("Failed to initialize bookmarker: %v", err)
			}
		}

		log.Info("Reading %s", r.Path)
		r.readFile(reader)
		log.Debug("Reader returned")
		break
	}

}

func (r *ReaderLoop) Stop() {
	r.stop = true
}

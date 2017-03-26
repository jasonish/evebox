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

package evereader

import (
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"io"
	"sync"
	"time"
)

const BATCH_SIZE = 1000

// EveFileProcessor processes eve files by reading events from reader, applying
// any filters then sending the events to an event sink.
type EveFileProcessor struct {
	Filename          string
	BookmarkDirectory string
	Sink              core.EveEventSink

	filters []eve.EveFilter

	customFields map[string]interface{}

	bookmarker *Bookmarker

	// Total number of events processed.
	count uint64

	// Flag to set to stop processing.
	stop bool

	wg sync.WaitGroup

	// Internal metrics counting.
	lastStatCount uint64
	lastStatTime  time.Time
	eofs          uint64
}

func (p *EveFileProcessor) AddFilter(filter eve.EveFilter) {
	p.filters = append(p.filters, filter)
}

func (p *EveFileProcessor) AddCustomField(field string, value interface{}) {
	if p.customFields == nil {
		p.customFields = make(map[string]interface{})
	}
	p.customFields[field] = value
}

func (p *EveFileProcessor) Start() {
	p.lastStatTime = time.Now()
	p.wg.Add(1)
	go p.Run()
}

func (p *EveFileProcessor) Stop() {
	p.stop = true
	p.wg.Wait()
}

func (p *EveFileProcessor) Run() {
	// Outer loop retries opening the file in case of failures...
	for {
		var bookmarker *Bookmarker

		reader, err := NewFollowingReader(p.Filename)
		if err != nil {
			log.Warning("Failed to open %s (will try again): %v",
				p.Filename, err)
			goto Retry
		}

		bookmarker, err = NewBookmarker(reader, p.BookmarkDirectory)
		if err != nil {
			log.Warning("Failed to get bookmarker (will try again): %v", err)
			reader.Close()
			goto Retry
		}

		if err := p.process(reader, bookmarker); err != nil {
			log.Error("Processing error, will retry: %v", err)
			goto Retry
		}

	Retry:
		if p.stop {
			break
		}
		time.Sleep(1 * time.Second)
	}

	p.wg.Done()
}

func (p *EveFileProcessor) process(reader *FollowingReader, bookmarker *Bookmarker) error {

	count := uint64(0)

	for {
		eof := false

		event, err := reader.Next()
		if err != nil {
			if err == io.EOF {
				eof = true
				p.eofs++
			} else {
				return err
			}
		}

		if !eof {
			for _, filter := range p.filters {
				filter.Filter(event)
			}
			p.addCustomFields(event)
			if err := p.Sink.Submit(event); err != nil {
				log.Error("Failed to submit event: %v", err)
				continue
			}
			count++
		}

		// On every EOF or batch size, commit.
		if (eof && count > 0) || (count > 0 && count%BATCH_SIZE == 0) {
			start := time.Now()
			if err := p.commit(); err != nil {
				log.Error("Commit failed: %v", err)
				return err
			}
			log.Debug("Committed %d events in %v", count,
				time.Now().Sub(start))
			bookmarker.UpdateBookmark()
			p.count += count
			count = 0
		}

		// Print stats.
		now := time.Now()
		if now.Sub(p.lastStatTime).Seconds() > 60 {
			log.Info("Total: %d; last minute: %d; EOFs: %d",
				p.count,
				p.count-p.lastStatCount,
				p.eofs)
			p.lastStatCount = p.count
			p.lastStatTime = now
			p.eofs = 0
		}

		if p.stop {
			break
		}

		// If eof, sleep for a moment.
		if eof {
			time.Sleep(1 * time.Second)
		}
	}

	return nil
}

func (p *EveFileProcessor) commit() error {
	for {
		_, err := p.Sink.Commit()
		if err == nil {
			return nil
		}
		if p.stop {
			return err
		}
		log.Error("Failed to commit events, will try again: %v", err)
		time.Sleep(1 * time.Second)
		continue
	}
}

func (p *EveFileProcessor) addCustomFields(event eve.EveEvent) {
	for key := range p.customFields {
		event[key] = p.customFields[key]
	}
}

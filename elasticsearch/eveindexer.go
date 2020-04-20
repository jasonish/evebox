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

package elasticsearch

import (
	"encoding/json"
	"fmt"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"github.com/oklog/ulid"
	"github.com/pkg/errors"
	"math/rand"
	"net/http"
	"sync"
	"time"
)

const atTimestampFormat = "2006-01-02T15:04:05.999Z"

var templateCheckLock sync.Mutex

var entropyLock sync.Mutex
var lastSeed int64 = 0

var initTemplateOnce sync.Once

type BulkEveIndexer struct {
	es      *ElasticSearch
	queued  uint
	buf     []byte

	// The entropy source for ulid generation.
	entropy *rand.Rand
}

func NewIndexer(es *ElasticSearch) *BulkEveIndexer {
	indexer := BulkEveIndexer{
		es: es,
	}
	indexer.initEntropy()
	initTemplateOnce.Do(indexer.initTemplate)
	return &indexer
}

func (i *BulkEveIndexer) initTemplate() {
	exists, err := i.es.TemplateExists(i.es.EventIndexPrefix)
	if err != nil {
		log.Error("Failed to check if template exists: %v", err)
		return
	}
	if !exists || i.es.config.ForceTemplate {
		i.es.LoadTemplate()
	}
}

func (i *BulkEveIndexer) initEntropy() {
	entropyLock.Lock()
	defer entropyLock.Unlock()

	seed := lastSeed

	for seed == lastSeed {
		seed = time.Now().UnixNano()
	}
	lastSeed = seed

	i.entropy = rand.New(rand.NewSource(seed))
}

func (i *BulkEveIndexer) DecodeResponse(response *http.Response) (*Response, error) {
	return DecodeResponse(response)
}

func (i *BulkEveIndexer) Submit(event eve.EveEvent) error {

	timestamp := event.Timestamp()
	event["@timestamp"] = timestamp.UTC().Format(atTimestampFormat)
	index := fmt.Sprintf("%s-%s", i.es.EventIndexPrefix,
		timestamp.UTC().Format("2006.01.02"))

	header := BulkCreateHeader{}
	header.Create.Index = index

	id := ulid.MustNew(ulid.Timestamp(timestamp), i.entropy).String()
	header.Create.Id = id

	rheader, _ := json.Marshal(header)
	revent, _ := json.Marshal(event)

	i.buf = append(i.buf, rheader...)
	i.buf = append(i.buf, []byte("\n")...)
	i.buf = append(i.buf, revent...)
	i.buf = append(i.buf, []byte("\n")...)

	i.queued++

	return nil
}

func (i *BulkEveIndexer) Commit() (interface{}, error) {

	// Check if the template exists for the index before adding events.
	// If not, try to install it.
	//
	// This is wrapped in lock so only on go-routine ends up doing this.
	//
	// Probably need to rethink this, perhaps do it on startup. But periodic
	// checks are required in case Elastic Search was re-installed or something
	// and the templates lost.
	templateCheckLock.Lock()
	exists, err := i.es.TemplateExists(i.es.EventIndexPrefix)
	if err != nil {
		log.Error("Failed to check if template %s exists: %v",
			i.es.EventIndexPrefix, err)
		templateCheckLock.Unlock()
		return nil, errors.Errorf("no template installed for configured index")
	} else if !exists {
		log.Warning("Template %s does not exist, will create.",
			i.es.EventIndexPrefix)
		err := i.es.LoadTemplate()
		if err != nil {
			log.Error("Failed to install template: %v", err)
			templateCheckLock.Unlock()
			return nil, errors.Errorf("failed to install template for configured index")
		}
	}
	templateCheckLock.Unlock()

	if len(i.buf) > 0 {
		response, err := i.es.httpClient.PostBytes("_bulk",
			"application/json", i.buf)
		if err != nil {
			return nil, err
		}
		i.buf = i.buf[:0]
		i.queued = 0
		bulkResponse, err := i.DecodeResponse(response)
		return bulkResponse, err
	}

	return nil, nil
}

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
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/log"
)

type DataStore struct {
	*EventQueryService
	*EventService

	es *ElasticSearch
}

func NewDataStore(es *ElasticSearch) (*DataStore, error) {

	eventQueryService := NewEventQueryService(es)
	eventService := NewEventService(es)

	datastore := DataStore{
		EventQueryService: eventQueryService,
		EventService:      eventService,
		es:                es,
	}

	return &datastore, nil
}

func (d *DataStore) GetEveEventSink() core.EveEventSink {
	return NewIndexer(d.es)
}

func (d *DataStore) FindFlow(flowId uint64, proto string, timestamp string, srcIp string, destIp string) (interface{}, error) {

	query := NewEventQuery()
	query.Size = 1

	query.EventType("flow")
	query.AddFilter(TermQuery("flow_id", flowId))
	query.AddFilter(TermQuery("proto", proto))
	query.AddFilter(RangeLte("flow.start", timestamp))
	query.AddFilter(RangeGte("flow.end", timestamp))
	query.ShouldHaveIp(srcIp, d.es.keyword)
	query.ShouldHaveIp(destIp, d.es.keyword)

	response, err := d.es.Search(query)
	if err != nil {
		log.Error("%v", err)
		return nil, err
	}

	return response.Hits.Hits, nil
}

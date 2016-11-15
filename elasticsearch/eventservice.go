/* Copyright (c) 2014-2015 Jason Ish
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
	"fmt"
	"github.com/jasonish/evebox/log"
	"time"
)

type GetEventByIdQuery struct {
	Query struct {
		Bool struct {
			Filter struct {
				Term struct {
					ID string `json:"_id"`
				} `json:"term"`
			} `json:"filter"`
		} `json:"bool"`
	} `json:"query"`
}

type EventService struct {
	es *ElasticSearch

	// Keyword for keyword queryes. Should be "raw" or "keyword".
	keyword string
}

func NewEventService(es *ElasticSearch) *EventService {

	keyword, err := es.GetKeywordType("")
	if err != nil {
		log.Warning("Failed to determine Elastic Search keyword type, using 'keyword'")
		keyword = "keyword"
	} else {
		log.Info("Using Elastic Search keyword type %s.", keyword)
	}

	eventService := &EventService{
		es:      es,
		keyword: keyword,
	}
	return eventService
}

// GetEventById returns the event with the given ID. If not event is found
// nil will be returned for the event and error will not be set.
func (s *EventService) GetEventById(id string) (map[string]interface{}, error) {
	query := GetEventByIdQuery{}
	query.Query.Bool.Filter.Term.ID = id
	result, err := s.es.Search(query)
	if err != nil {
		return nil, err
	}
	if len(result.Hits.Hits) > 0 {
		return result.Hits.Hits[0], nil
	}

	// No event found.
	return nil, nil
}

type EventQuery struct {
	Query struct {
		Bool struct {
			Filter []interface{} `json:"filter"`
		} `json:"bool"`
	} `json:"query"`
	Size int64         `json:"size"`
	Sort []interface{} `json:"sort,omitempty"`
}

func (q *EventQuery) AddFilter(filter interface{}) {
	q.Query.Bool.Filter = append(q.Query.Bool.Filter, filter)
}

func (q *EventQuery) AddTimeRangeFilter(timeRange string) {
	duration, _ := time.ParseDuration(fmt.Sprintf("-%s", timeRange))
	then := time.Now().Add(duration)
	q.AddFilter(map[string]interface{}{
		"range": map[string]interface{}{
			"@timestamp": m{
				"gte": then,
			},
		},
	})
}

func (s *EventService) Inbox(options map[string]interface{}) (map[string]interface{}, error) {
	query := EventQuery{}

	query.AddFilter(ExistsQuery("event_type"))
	query.AddFilter(TermQuery("event_type", "alert"))

	if queryString, ok := options["queryString"]; ok {
		query.AddFilter(map[string]interface{}{
			"query_string": map[string]interface{}{
				"query": queryString,
			},
		})
	}

	if timeRange, ok := options["timeRange"]; ok {
		query.AddTimeRangeFilter(timeRange.(string))
	}

	log.Println(ToJson(query))

	results, err := s.es.Search(query)
	if err != nil {
		log.Error("%v", err)
	} else {
		log.Println(ToJson(results))
	}

	return nil, nil
}

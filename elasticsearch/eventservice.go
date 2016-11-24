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
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/log"
	"io"
	"io/ioutil"
	"strings"
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
}

func NewEventService(es *ElasticSearch) *EventService {
	eventService := &EventService{
		es: es,
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
			Filter  []interface{} `json:"filter"`
			MustNot []interface{} `json:"must_not,omitempty"`
			Should  []interface{} `json:"should,omitempty"`

			// Should be an integer, but we make it an interface so
			// its not included if not set.
			MinimumShouldMatch interface{} `json:"minimum_should_match,omitempty"`
		} `json:"bool"`
	} `json:"query"`
	Size int64                  `json:"size"`
	Sort []interface{}          `json:"sort,omitempty"`
	Aggs map[string]interface{} `json:"aggs,omitempty"`
}

func NewEventQuery() EventQuery {
	query := EventQuery{}
	query.AddFilter(ExistsQuery("event_type"))
	query.Sort = []interface{}{
		Sort("@timestamp", "desc"),
	}
	query.Aggs = map[string]interface{}{}
	return query
}

func (q *EventQuery) EventType(eventType string) {
	q.AddFilter(TermQuery("event_type", eventType))
}

func (q *EventQuery) ShouldHaveIp(addr string, keyword string) {
	if strings.HasSuffix(addr, ".") {
		q.Should(KeywordPrefixQuery("src_ip", addr, keyword))
		q.Should(KeywordPrefixQuery("dest_ip", addr, keyword))
	} else {
		q.Should(KeywordTermQuery("src_ip", addr, keyword))
		q.Should(KeywordTermQuery("dest_ip", addr, keyword))
	}
	q.Query.Bool.MinimumShouldMatch = 1
}

func (q *EventQuery) AddFilter(filter interface{}) {
	q.Query.Bool.Filter = append(q.Query.Bool.Filter, filter)
}

func (q *EventQuery) Should(filter interface{}) {
	q.Query.Bool.Should = append(q.Query.Bool.Should, filter)
}

func (q *EventQuery) MustNot(query interface{}) {
	q.Query.Bool.MustNot = append(q.Query.Bool.MustNot, query)
}

func (q *EventQuery) SortBy(field string, order string) *EventQuery {
	q.Sort = append(q.Sort, Sort(field, order))
	return q
}

func (q *EventQuery) AddTimeRangeFilter(timeRange string) error {
	duration, err := time.ParseDuration(fmt.Sprintf("-%s", timeRange))
	if err != nil {
		return err
	}
	then := time.Now().Add(duration)
	q.AddFilter(map[string]interface{}{
		"range": map[string]interface{}{
			"@timestamp": m{
				"gte": then,
			},
		},
	})

	return nil
}

func (s *EventService) asKeyword(keyword string) string {
	return fmt.Sprintf("%s.%s", keyword, s.es.keyword)
}

// AddTagsToEvent will add the given tags to the event referenced by ID.
func (s *EventService) AddTagsToEvent(id string, addTags []string) error {

	raw, err := s.GetEventById(id)
	if err != nil {
		return err
	}

	event := JsonMap(raw)
	tags := event.GetMap("_source").GetAsStrings("tags")

	for _, tag := range addTags {
		if !StringSliceContains(tags, tag) {
			tags = append(tags, tag)
		}
	}

	s.es.PartialUpdate(event.GetString("_index"), event.GetString("_type"),
		event.GetString("_id"), map[string]interface{}{
			"tags": tags,
		})

	return nil
}

func (s *EventService) RemoveTagsFromEvent(id string, rmTags []string) error {

	raw, err := s.GetEventById(id)
	if err != nil {
		return err
	}

	event := JsonMap(raw)
	currentTags := event.GetMap("_source").GetAsStrings("tags")
	tags := make([]string, 0)

	for _, tag := range currentTags {
		if !StringSliceContains(rmTags, tag) {
			tags = append(tags, tag)
		}
	}

	s.es.PartialUpdate(event.GetString("_index"), event.GetString("_type"),
		event.GetString("_id"), map[string]interface{}{
			"tags": tags,
		})

	return nil
}

// AddTagsToAlertGroup adds the provided tags to all alerts that match the
// provided alert group parameters.
func (s *EventService) AddTagsToAlertGroup(p core.AlertGroupQueryParams, tags []string) error {

	mustNot := []interface{}{}
	for _, tag := range tags {
		mustNot = append(mustNot, TermQuery("tags", tag))
	}

	query := m{
		"query": m{
			"bool": m{
				"filter": l{
					ExistsQuery("event_type"),
					KeywordTermQuery("event_type", "alert", s.es.keyword),
					RangeQuery{
						Field: "timestamp",
						Gte:   p.MinTimestamp,
						Lte:   p.MaxTimestamp,
					},
					KeywordTermQuery("src_ip", p.SrcIP, s.es.keyword),
					KeywordTermQuery("dest_ip", p.DstIP, s.es.keyword),
					TermQuery("alert.signature_id", p.SignatureID),
				},
				"must_not": mustNot,
			},
		},
		"_source": "tags",
		"sort": l{
			"_doc",
		},
		"size": 10000,
	}

	searchResponse, err := s.es.SearchScroll(query, "1m")
	if err != nil {
		log.Error("Failed to initialize scroll: %v", err)
		return err
	}

	scrollID := searchResponse.ScrollId

	for {

		log.Debug("Search response total: %d; hits: %d",
			searchResponse.Hits.Total, len(searchResponse.Hits.Hits))

		if len(searchResponse.Hits.Hits) == 0 {
			break
		}

		// We do this in a retry loop as some documents may fail to be
		// updated. Most likely rejected due to max thread count or
		// something.
		maxRetries := 5
		retries := 0
		for {
			retry, err := bulkUpdateTags(s.es, searchResponse.Hits.Hits,
				tags, nil)
			if err != nil {
				log.Error("BulkAddTags failed: %v", err)
				return err
			}
			if !retry {
				break
			}
			retries++
			if retries > maxRetries {
				log.Warning("Errors occurred archive events, not all events may have been archived.")
				break
			}
		}

		// Get next set of events to archive.
		searchResponse, err = s.es.Scroll(scrollID, "1m")
		if err != nil {
			log.Error("Failed to fetch from scroll: %v", err)
			return err
		}

	}

	response, err := s.es.DeleteScroll(scrollID)
	if err != nil {
		log.Error("Failed to delete scroll id: %v", err)
	}
	io.Copy(ioutil.Discard, response.Body)

	s.es.Refresh()

	return nil
}

// ArchiveAlertGroup is a specialization of AddTagsToAlertGroup.
func (s *EventService) ArchiveAlertGroup(p core.AlertGroupQueryParams) error {
	return s.AddTagsToAlertGroup(p, []string{"archived", "evebox.archived"})
}

// EscalateAlertGroup is a specialization of AddTagsToAlertGroup.
func (s *EventService) EscalateAlertGroup(p core.AlertGroupQueryParams) error {
	return s.AddTagsToAlertGroup(p, []string{"escalated", "evebox.escalated"})
}

// RemoveTagsFromAlertGroup removes the given tags from all alerts matching
// the provided parameters.
func (s *EventService) RemoveTagsFromAlertGroup(p core.AlertGroupQueryParams, tags []string) error {

	filter := []interface{}{
		ExistsQuery("event_type"),
		KeywordTermQuery("event_type", "alert", s.es.keyword),
		RangeQuery{
			Field: "timestamp",
			Gte:   p.MinTimestamp,
			Lte:   p.MaxTimestamp,
		},
		KeywordTermQuery("src_ip", p.SrcIP, s.es.keyword),
		KeywordTermQuery("dest_ip", p.DstIP, s.es.keyword),
		TermQuery("alert.signature_id", p.SignatureID),
	}

	for _, tag := range tags {
		filter = append(filter, TermQuery("tags", tag))
	}

	query := m{
		"query": m{
			"bool": m{
				"filter": filter,
			},
		},
		"_source": "tags",
		"sort": l{
			"_doc",
		},
		"size": 10000,
	}

	log.Println(ToJson(query))

	searchResponse, err := s.es.SearchScroll(query, "1m")
	if err != nil {
		log.Error("Failed to initialize scroll: %v", err)
		return err
	}

	scrollID := searchResponse.ScrollId

	for {

		log.Debug("Search response total: %d; hits: %d",
			searchResponse.Hits.Total, len(searchResponse.Hits.Hits))

		if len(searchResponse.Hits.Hits) == 0 {
			break
		}

		// We do this in a retry loop as some documents may fail to be
		// updated. Most likely rejected due to max thread count or
		// something.
		maxRetries := 5
		retries := 0
		for {
			retry, err := bulkUpdateTags(s.es, searchResponse.Hits.Hits,
				nil, tags)
			if err != nil {
				log.Error("BulkAddTags failed: %v", err)
				return err
			}
			if !retry {
				break
			}
			retries++
			if retries > maxRetries {
				log.Warning("Errors occurred archive events, not all events may have been archived.")
				break
			}
		}

		// Get next set of events to archive.
		searchResponse, err = s.es.Scroll(scrollID, "1m")
		if err != nil {
			log.Error("Failed to fetch from scroll: %v", err)
			return err
		}

	}

	response, err := s.es.DeleteScroll(scrollID)
	if err != nil {
		log.Error("Failed to delete scroll id: %v", err)
	}
	io.Copy(ioutil.Discard, response.Body)

	s.es.Refresh()

	return nil
}

func TopHitsAgg(field string, order string, size int64) interface{} {
	return map[string]interface{}{
		"top_hits": map[string]interface{}{
			"sort": []map[string]interface{}{
				map[string]interface{}{
					field: map[string]interface{}{
						"order": order,
					},
				},
			},
			"size": size,
		},
	}
}

func (s *EventService) FindNetflow(options core.EventQueryOptions, sortBy string, order string) (interface{}, error) {

	size := int64(10)

	if options.Size > 0 {
		size = options.Size
	}

	if order == "" {
		order = "desc"
	}

	query := NewEventQuery()
	query.AddFilter(TermQuery("event_type", "netflow"))

	if options.TimeRange != "" {
		query.AddTimeRangeFilter(options.TimeRange)
	}

	if options.QueryString != "" {
		query.AddFilter(QueryString(options.QueryString))
	}

	if sortBy != "" {
		query.Aggs["agg"] = TopHitsAgg(sortBy, order, size)
	} else {
		query.Size = size
	}

	log.Println(ToJsonPretty(query))

	response, err := s.es.Search(query)
	if err != nil {
		return nil, err
	}

	// Unwrap response.
	hits := response.Aggregations.GetMap("agg").GetMap("hits").Get("hits")

	return map[string]interface{}{
		"data": hits,
	}, nil
}

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
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/util"
	"sort"
	"time"
)

// AlertGroupResultEntry is a single entry in the list of alert group responses.
// Its a group rather than an individual alert as it represents many alerts
// that have been grouped together by some parameters such as signature,
// source and destination.
//
// It provides enough information to act on the alert group such as archiving
// or escalating all the alerts in the group.
type AlertGroupResultEntry struct {
	Count          int64                  `json:"count"`
	Event          map[string]interface{} `json:"event"`
	MaxTs          string                 `json:"maxTs"`
	MinTs          string                 `json:"minTs"`
	EscalatedCount int64                  `json:"escalatedCount"`

	time time.Time
}

// Time returns the timestamp of the alert group as a time.Time value.
func (a AlertGroupResultEntry) Time() time.Time {
	if a.time.IsZero() {
		a.time, _ = eve.ParseTimestamp(a.MaxTs)
	}
	return a.time
}

// AlertGroupResultSet is a list of AlertGroupResultEntry's including an
// interface implementing for sorting.
type AlertGroupResultSet []AlertGroupResultEntry

func (a AlertGroupResultSet) Len() int {
	return len(a)
}

func (a AlertGroupResultSet) Less(i, j int) bool {
	return a[i].Time().Before(a[j].Time())
}

func (a AlertGroupResultSet) Swap(i, j int) {
	a[i], a[j] = a[j], a[i]
}

// AlertGroupResult is the "wrapper" type for the returned result set in case
// additional data is required.
type AlertGroupResult struct {
	AlertGroups AlertGroupResultSet `json:"alerts"`
}

// Return a 3 tuple aggregation: signature, source, dest...
func (s *DataStore) get3TupleAggs() map[string]interface{} {

	size := 10000

	aggs := map[string]interface{}{
		"signatures": m{
			"terms": m{
				"field": "alert.signature_id",
				"size":  size,
			},
			"aggs": m{
				"sources": m{
					"terms": m{
						"field": s.es.FormatKeyword("src_ip"),
						"size":  size,
					},
					"aggs": m{
						"destinations": m{
							"terms": m{
								"field": s.es.FormatKeyword("dest_ip"),
								"size":  size,
							},
							"aggs": m{
								"newest": m{
									"top_hits": m{
										"sort": l{
											Sort("@timestamp", "desc"),
										},
										"size": 1,
									},
								},
								"oldest": m{
									"top_hits": m{
										"sort": l{
											Sort("@timestamp", "asc"),
										},
										"size": 1,
									},
								},
								"escalated": m{
									"filter": m{
										"term": m{
											"tags": "escalated",
										},
									},
								},
							},
						},
					},
				},
			},
		},
	}
	return aggs
}

func (s *DataStore) AlertQuery(options core.AlertQueryOptions) (interface{}, error) {

	query := NewEventQuery()

	// Limit to alerts.
	query.AddFilter(TermQuery("event_type", "alert"))

	// Set must have tags, for example to get escalated alerts.
	for _, tag := range options.MustHaveTags {
		query.AddFilter(TermQuery("tags", tag))
	}

	// Set must not have tags. For example, the inbox must not have
	// archive tags set.
	for _, tag := range options.MustNotHaveTags {
		query.MustNot(TermQuery("tags", tag))
	}

	if options.QueryString != "" {
		query.AddFilter(QueryString(options.QueryString))
	}

	if options.TimeRange != "" {
		query.AddTimeRangeFilter(options.TimeRange)
	} else {
		if !options.MaxTs.IsZero() {
			query.AddFilter(RangeLte("@timestamp",
				eve.FormatTimestampUTC(options.MaxTs)))
		}
		if !options.MinTs.IsZero() {
			query.AddFilter(RangeGte("@timestamp",
				eve.FormatTimestampUTC(options.MinTs)))
		}
	}

	// Set the aggs for grouping by sig, source, then dest...
	query.Aggs = s.get3TupleAggs()

	results, err := s.es.Search(query)
	if err != nil {
		return nil, err
	}

	alertGroups := AlertGroupResultSet{}

	aggs := util.JsonMap(results.Aggregations)
	signatures := aggs.GetMap("signatures")
	for _, bucket0 := range signatures.GetMapList("buckets") {
		sources := bucket0.GetMap("sources")
		for _, bucket1 := range sources.GetMapList("buckets") {
			destinations := bucket1.GetMap("destinations")
			for _, bucket2 := range destinations.GetMapList("buckets") {

				alertGroup := AlertGroupResultEntry{}
				alertGroup.Count, _ = bucket2.Get("doc_count").(json.Number).Int64()
				alertGroup.EscalatedCount, _ = bucket2.GetMap("escalated").Get("doc_count").(json.Number).Int64()

				minEvent := bucket2.GetMap("oldest").GetMap("hits").GetMapList("hits")[0]
				maxEvent := bucket2.GetMap("newest").GetMap("hits").GetMapList("hits")[0]

				alertGroup.MinTs = minEvent.GetMap("_source").GetString("@timestamp")
				alertGroup.MaxTs = maxEvent.GetMap("_source").GetString("@timestamp")

				alertGroup.Event = maxEvent

				if maxEvent["_source"].(map[string]interface{})["tags"] == nil {
					maxEvent["_source"].(map[string]interface{})["tags"] = []string{}
				}

				alertGroups = append(alertGroups, alertGroup)
			}
		}
	}

	sort.Sort(sort.Reverse(alertGroups))

	return AlertGroupResult{AlertGroups: alertGroups}, nil
}

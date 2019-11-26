/* Copyright (c) 2016-2019 Jason Ish
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
)

// AlertGroupList is a list of AlertGroup's including an
// interface implementing for sorting.
type AlertGroupList []core.AlertGroup

func (a AlertGroupList) Len() int {
	return len(a)
}

func (a AlertGroupList) Less(i, j int) bool {
	return a[i].Time().Before(a[j].Time())
}

func (a AlertGroupList) Swap(i, j int) {
	a[i], a[j] = a[j], a[i]
}

// Return a 3 tuple aggregation: signature, source, dest...
func (s *DataStore) get3TupleAggs() map[string]interface{} {

	size := 10000

	aggs := map[string]interface{}{
		"signatures": map[string]interface{}{
			"terms": map[string]interface{}{
				"field": "alert.signature_id",
				"size":  size,
			},
			"aggs": map[string]interface{}{
				"sources": map[string]interface{}{
					"terms": map[string]interface{}{
						"field": s.es.FormatKeyword("src_ip"),
						"size":  size,
					},
					"aggs": map[string]interface{}{
						"destinations": map[string]interface{}{
							"terms": map[string]interface{}{
								"field": s.es.FormatKeyword("dest_ip"),
								"size":  size,
							},
							"aggs": map[string]interface{}{
								"newest": map[string]interface{}{
									"top_hits": map[string]interface{}{
										"sort": []interface{}{
											Sort("@timestamp", "desc"),
										},
										"size": 1,
									},
								},
								"oldest": map[string]interface{}{
									"top_hits": map[string]interface{}{
										"sort": []interface{}{
											Sort("@timestamp", "asc"),
										},
										"size": 1,
									},
								},
								"escalated": map[string]interface{}{
									"filter": map[string]interface{}{
										"term": map[string]interface{}{
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

func (s *DataStore) AlertQuery(options core.AlertQueryOptions) ([]core.AlertGroup, error) {

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

	alertGroups := AlertGroupList{}

	aggs := util.JsonMap(results.Aggregations)
	signatures := aggs.GetMap("signatures")
	for _, bucket0 := range signatures.GetMapList("buckets") {
		sources := bucket0.GetMap("sources")
		for _, bucket1 := range sources.GetMapList("buckets") {
			destinations := bucket1.GetMap("destinations")
			for _, bucket2 := range destinations.GetMapList("buckets") {

				alertGroup := core.AlertGroup{}
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

	return alertGroups, nil
}

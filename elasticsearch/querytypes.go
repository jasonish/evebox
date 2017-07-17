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
	"strings"
	"time"
)

type Script struct {
	Lang   string                 `json:"lang,omitempty"`
	Inline string                 `json:"inline,omitempty"`
	Params map[string]interface{} `json:"params,omitempty"`
}

type Bool struct {
	Filter  []interface{} `json:"filter,omitempty"`
	MustNot []interface{} `json:"must_not,omitempty"`
	Should  []interface{} `json:"should,omitempty"`

	// Should be an integer, but we make it an interface so
	// its not included if not set.
	MinimumShouldMatch interface{} `json:"minimum_should_match,omitempty"`
}

type Query struct {
	Bool *Bool `json:"bool,omitempty"`
}

// EventQuery is a type for building up an Elastic Search event query.
type EventQuery struct {
	//Query struct {
	//	//Bool struct {
	//	//	Filter  []interface{} `json:"filter,omitempty"`
	//	//	MustNot []interface{} `json:"must_not,omitempty"`
	//	//	Should  []interface{} `json:"should,omitempty"`
	//	//
	//	//	// Should be an integer, but we make it an interface so
	//	//	// its not included if not set.
	//	//	MinimumShouldMatch interface{} `json:"minimum_should_match,omitempty"`
	//	//} `json:"bool,omitempty"`
	//	Bool *Bool `json:"bool,omitempty"`
	//} `json:"query,omitempty"`
	Query  *Query                 `json:"query,omitempty"`
	Script *Script                `json:"script,omitempty"`
	Size   int64                  `json:"size,omitempty"`
	Sort   []interface{}          `json:"sort,omitempty"`
	Aggs   map[string]interface{} `json:"aggs,omitempty"`
}

func NewEventQuery() EventQuery {
	query := EventQuery{}
	query.AddFilter(ExistsQuery("event_type"))

	// This is the default sort order. A SortBy() call will replace this.
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
	if q.Query == nil {
		q.Query = &Query{}
	}
	if q.Query.Bool == nil {
		q.Query.Bool = &Bool{}
	}
	q.Query.Bool.Filter = append(q.Query.Bool.Filter, filter)
}

func (q *EventQuery) Should(filter interface{}) {
	q.Query.Bool.Should = append(q.Query.Bool.Should, filter)
}

func (q *EventQuery) MustNot(query interface{}) {
	q.Query.Bool.MustNot = append(q.Query.Bool.MustNot, query)
}

func (q *EventQuery) SortBy(field string, order string) *EventQuery {
	q.Sort = []interface{}{
		Sort(field, order),
	}
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
			"@timestamp": map[string]interface{}{
				"gte": then,
			},
		},
	})

	return nil
}

func ExistsQuery(field string) interface{} {
	return map[string]interface{}{
		"exists": map[string]interface{}{
			"field": field,
		},
	}
}

func TermQuery(field string, value interface{}) map[string]interface{} {
	return map[string]interface{}{
		"term": map[string]interface{}{
			field: value,
		},
	}
}

func PrefixQuery(field string, value interface{}) map[string]interface{} {
	return map[string]interface{}{
		"prefix": map[string]interface{}{
			field: value,
		},
	}
}

func KeywordTermQuery(field string, value string, suffix string) map[string]interface{} {
	term := field
	if suffix != "" {
		term = fmt.Sprintf("%s.%s", field, suffix)
	}
	return TermQuery(term, value)
}

func KeywordPrefixQuery(field string, value string, suffix string) map[string]interface{} {
	term := field
	if suffix != "" {
		term = fmt.Sprintf("%s.%s", field, suffix)
	}
	return PrefixQuery(term, value)
}

func QueryString(query string) map[string]interface{} {
	return map[string]interface{}{
		"query_string": map[string]interface{}{
			"query":            query,
			"default_operator": "AND",
		},
	}
}

func Sort(field string, order string) map[string]interface{} {
	return map[string]interface{}{
		field: map[string]interface{}{
			"order": order,
		},
	}
}

func Range(rangeType string, field string, value interface{}) interface{} {
	return map[string]interface{}{
		"range": map[string]interface{}{
			field: map[string]interface{}{
				rangeType: value,
			},
		},
	}
}

func RangeGte(field string, value interface{}) interface{} {
	return Range("gte", field, value)
}

func RangeLte(field string, value interface{}) interface{} {
	return Range("lte", field, value)
}

type RangeQuery struct {
	Field string
	Gte   string
	Lte   string
}

func (r RangeQuery) MarshalJSON() ([]byte, error) {
	values := map[string]string{}
	if r.Gte != "" {
		values["gte"] = r.Gte
	}
	if r.Lte != "" {
		values["lte"] = r.Lte
	}

	rangeq := map[string]interface{}{
		"range": map[string]interface{}{
			r.Field: values,
		},
	}

	return json.Marshal(rangeq)
}

func TopHitsAgg(field string, order string, size int64) interface{} {
	return map[string]interface{}{
		"top_hits": map[string]interface{}{
			"sort": []map[string]interface{}{
				map[string]interface{}{
					field: map[string]interface{}{
						"order": order,

						// Probably need to make this
						// a function parameter.
						"unmapped_type": "long",
					},
				},
			},
			"size": size,
		},
	}
}

// BulkCreateHeader represents the JSON used to prefix a document to be indexed
// in the bulk request.
type BulkCreateHeader struct {
	Create struct {
		Index string `json:"_index"`
		Type  string `json:"_type"`
		Id    string `json:"_id"`
	} `json:"create"`
}

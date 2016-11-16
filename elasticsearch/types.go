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
	"strconv"
	"strings"
)

// Type alias for a map[string] - helper for building up JSON.
type m map[string]interface{}

// Type alias for an interface slice - helper for building up JSON lists.
type l []interface{}

// PingResponse represents the response to an Elastic Search ping (GET /).
type PingResponse struct {
	Name        string `json:"name"`
	ClusterName string `json:"cluster_name"`
	Version     struct {
		Number string `json:"number"`
	} `json:"version"`
	Tagline string `json:"tagline"`
}

// MajorVersion returns the major version of Elastic Search as found
// in the PingResponse.
func (p PingResponse) MajorVersion() int64 {
	version := p.Version.Number
	parts := strings.Split(version, ".")
	major, err := strconv.ParseInt(parts[0], 10, 64)
	if err != nil {
		return -1
	}
	return major

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

// Struct representing a response to a _bulk request.
type BulkResponse struct {
	Took   uint64                   `json:"took"`
	Errors bool                     `json:"errors"`
	Items  []map[string]interface{} `json:"items"`
}

type Hits struct {
	Total uint64                   `json:"total"`
	Hits  []map[string]interface{} `json:"hits"`
	//Hits []Hit `json:"hits"`
}

type SearchResponse struct {
	Shards       map[string]interface{} `json:"_shards"`
	TimedOut     bool                   `json:"timed_out"`
	Took         uint64                 `json:"took"`
	Hits         Hits                   `json:"hits"`
	Aggregations map[string]interface{} `json:"aggregations"`
	ScrollId     string                 `json:"_scroll_id,omitempty"`
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

func KeywordTermQuery(field string, value string, suffix string) map[string]interface{} {
	return map[string]interface{}{
		"term": map[string]interface{}{
			fmt.Sprintf("%s.%s", field, suffix): value,
		},
	}
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

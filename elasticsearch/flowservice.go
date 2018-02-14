/* Copyright (c) 2018 Jason Ish
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
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
)

type FlowService struct {
	es *ElasticSearch
}

func NewFlowService(es *ElasticSearch) core.FlowService {
	return &FlowService{es: es}
}

func (s *FlowService) Histogram(options core.FlowHistogramOptions) interface{} {
	query := NewEventQuery()
	query.EventType("flow")
	query.Size = 0

	if options.QueryString != "" {
		query.AddFilter(QueryString(options.QueryString))
	}

	if options.TimeRange != "" {
		query.AddTimeRangeFilter(options.TimeRange)
	}

	if !options.MinTs.IsZero() {
		query.AddFilter(RangeGte("@timestamp",
			eve.FormatTimestampUTC(options.MinTs)))
	}

	if !options.MaxTs.IsZero() {
		query.AddFilter(RangeLte("@timestamp",
			eve.FormatTimestampUTC(options.MaxTs)))
	}

	agg := NewDateHistogram().Field("@timestamp")

	if options.Interval != "" {
		agg.Interval(options.Interval)
	} else {
		agg.Interval("1h")
	}

	for _, subAgg := range options.SubAggs {
		switch subAgg {
		case "app_proto":
			agg.AddAgg("app_proto", map[string]interface{}{
				"terms": map[string]interface{}{
					"field": s.es.FormatKeyword("app_proto"),
				},
			})
		case "bytes_toclient":
			agg.AddAgg("bytes_toclient",
				NewSumAggregation().Field("flow.bytes_toclient"))
		case "bytes_toserver":
			agg.AddAgg("bytes_toserver",
				NewSumAggregation().Field("flow.bytes_toserver"))
		case "pkts_toclient":
			agg.AddAgg("pkts_toclient",
				NewSumAggregation().Field("flow.pkts_toclient"))
		case "pkts_toserver":
			agg.AddAgg("pkts_toserver",
				NewSumAggregation().Field("flow.pkts_toserver"))
		default:
			log.Warning("Unknown flow histogram sub-agg: %s", subAgg)
		}
	}

	query.Aggs["histogram"] = agg

	response, _ := s.es.Search(query)

	data := map[string]interface{}{}
	data["raw"] = response

	values := []interface{}{}

	for _, bucket := range response.Aggregations.GetMap("histogram").GetMapList("buckets") {

		entry := map[string]interface{}{
			"key":    bucket.GetString("key_as_string"),
			"events": bucket.GetInt64("doc_count"),
		}

		appProto := bucket.GetMap("app_proto")
		if appProto != nil {
			appProtos := map[string]int64{}
			for _, proto := range appProto.GetMapList("buckets") {
				appProtos[proto.GetString("key")] = proto.GetInt64("doc_count")
			}
			entry["app_proto"] = appProtos
		}

		if bucket.HasKey("bytes_toclient") {
			entry["bytes_toclient"] = bucket.GetMap("bytes_toclient").GetInt64("value")
		}
		if bucket.HasKey("bytes_toserver") {
			entry["bytes_toserver"] = bucket.GetMap("bytes_toserver").GetInt64("value")
		}
		if bucket.HasKey("pkts_toclient") {
			entry["pkts_toclient"] = bucket.GetMap("pkts_toclient").GetInt64("value")
		}
		if bucket.HasKey("pkts_toserver") {
			entry["pkts_toserver"] = bucket.GetMap("pkts_toserver").GetInt64("value")
		}

		values = append(values, entry)
	}

	data["data"] = values

	return data
}

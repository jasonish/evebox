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
	"fmt"
	"github.com/jasonish/evebox/core"
	"strings"
	"time"
)

type ReportService struct {
	es *ElasticSearch
}

func NewReportService(es *ElasticSearch) *ReportService {
	return &ReportService{
		es: es,
	}
}

// ReportDnsRequestRrnames returns the top requests rrnames.
func (s *ReportService) ReportDnsRequestRrnames(options core.ReportOptions) (interface{}, error) {

	size := int64(10)

	query := NewEventQuery()

	query.AddFilter(TermQuery("event_type", "dns"))
	query.AddFilter(TermQuery("dns.type", "query"))
	query.SortBy("@timestamp", "desc")

	if options.TimeRange != "" {
		query.AddTimeRangeFilter(options.TimeRange)
	}

	if options.QueryString != "" {
		query.AddFilter(QueryString(options.QueryString))
	}

	if options.Size > 0 {
		size = options.Size
	}

	agg := map[string]interface{}{
		"terms": map[string]interface{}{
			"field": "dns.rrname.raw",
			"size":  size,
		},
	}
	query.Aggs["topRrnames"] = agg

	response, err := s.es.Search(query)
	if err != nil {
		return nil, err
	}

	data := make([]interface{}, 0)

	results := JsonMap(response.Aggregations["topRrnames"].(map[string]interface{}))
	for _, bucket := range results.GetMapList("buckets") {
		data = append(data, map[string]interface{}{
			"count": bucket.Get("doc_count"),
			"key":   bucket.Get("key"),
		})

	}

	return data, nil
}

// Alerts over time (histogram) for Ip.
func (s *ReportService) ReportIpAlertsOverTime(addr string, options core.ReportOptions) (interface{}, error) {

	query := NewEventQuery()
	query.AddFilter(TermQuery("event_type", "alert"))
	if strings.HasSuffix(addr, ".") {
		query.Should(KeywordPrefixQuery("src_ip", addr, s.es.keyword))
		query.Should(KeywordPrefixQuery("dest_ip", addr, s.es.keyword))
	} else {
		query.Should(KeywordTermQuery("src_ip", addr, s.es.keyword))
		query.Should(KeywordTermQuery("dest_ip", addr, s.es.keyword))
	}
	query.Query.Bool.MinimumShouldMatch = 1
	query.AddTimeRangeFilter(options.TimeRange)

	now := time.Now()
	duration, _ := time.ParseDuration(fmt.Sprintf("-%s", options.TimeRange))
	then := now.Add(duration)

	query.Aggs["alertsOverTime"] = map[string]interface{}{
		"date_histogram": map[string]interface{}{
			"field":         "@timestamp",
			"interval":      "1h",
			"min_doc_count": 0,
			"extended_bounds": map[string]interface{}{
				"min": then,
				"max": now,
			},
		},
	}

	response, err := s.es.Search(query)
	return response, err
}

func (s *ReportService) ReportHistogram(interval string, options core.ReportOptions) (interface{}, error) {

	query := NewEventQuery()

	if options.AddressFilter != "" {
		query.ShouldHaveIp(options.AddressFilter, s.es.keyword)
	}

	if options.QueryString != "" {
		query.AddFilter(QueryString(options.QueryString))
	}

	if options.TimeRange != "" {
		query.AddTimeRangeFilter(options.TimeRange)
	}

	if options.SensorFilter != "" {
		query.AddFilter(KeywordTermQuery("host", options.SensorFilter, s.es.keyword))
	}

	if options.EventType != "" {
		query.AddFilter(TermQuery("event_type", options.EventType))
	}

	if options.DnsType != "" {
		query.AddFilter(TermQuery("dns.type", options.DnsType))
	}

	query.Aggs["histogram"] = map[string]interface{}{
		"date_histogram": map[string]interface{}{
			"field":         "@timestamp",
			"interval":      interval,
			"min_doc_count": 0,
		},
	}

	if options.TimeRange != "" {
		now := time.Now()
		duration, _ := time.ParseDuration(fmt.Sprintf("-%s", options.TimeRange))
		then := now.Add(duration)

		query.Aggs["histogram"].(map[string]interface{})["date_histogram"].(map[string]interface{})["extended_bounds"] = map[string]interface{}{
			"min": then,
			"max": now,
		}
	}

	response, err := s.es.Search(query)
	if err != nil {
		return nil, err
	}

	// Unwrap response.
	data := []map[string]interface{}{}
	buckets := response.Aggregations.GetMap("histogram").GetMapList("buckets")
	for _, bucket := range buckets {
		data = append(data, map[string]interface{}{
			"key":           bucket.Get("key"),
			"count":         bucket.Get("doc_count"),
			"key_as_string": bucket.Get("key_as_string"),
		})
	}

	return map[string]interface{}{
		"data": data,
	}, nil
}

func (s *ReportService) ReportAlertAggs(agg string, options core.ReportOptions) (interface{}, error) {

	size := int64(10)

	query := NewEventQuery()
	query.EventType("alert")

	if options.QueryString != "" {
		query.AddFilter(QueryString(options.QueryString))
	}

	if options.AddressFilter != "" {
		query.ShouldHaveIp(options.AddressFilter, s.es.keyword)
	}

	if options.TimeRange != "" {
		err := query.AddTimeRangeFilter(options.TimeRange)
		if err != nil {
			return nil, err
		}
	}

	if options.Size > 0 {
		size = options.Size
	}

	switch agg {

	// Aggregations on keyword terms...
	case "src_ip":
		fallthrough
	case "dest_ip":
		fallthrough
	case "alert.category":
		fallthrough
	case "alert.signature":
		query.Aggs[agg] = map[string]interface{}{
			"terms": map[string]interface{}{
				"field": fmt.Sprintf("%s.%s", agg, s.es.keyword),
				"size":  size,
			},
		}

	// Aggregatiosn on number types.
	case "src_port":
		fallthrough
	case "dest_port":
		query.Aggs[agg] = map[string]interface{}{
			"terms": map[string]interface{}{
				"field": agg,
				"size":  size,
			},
		}

	default:
		return nil, fmt.Errorf("unknown aggregation: %s", agg)
	}

	response, err := s.es.Search(query)
	if err != nil {
		return nil, err
	}

	// Unwrap response.
	buckets := JsonMap(response.Aggregations[agg].(map[string]interface{})).GetMapList("buckets")
	data := []map[string]interface{}{}
	for _, bucket := range buckets {
		data = append(data, map[string]interface{}{
			"key":   bucket["key"],
			"count": bucket["doc_count"],
		})
	}

	return map[string]interface{}{
		"data": data,
	}, nil
}

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
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/util"
	"github.com/pkg/errors"
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

	results := util.JsonMap(response.Aggregations["topRrnames"].(map[string]interface{}))
	for _, bucket := range results.GetMapList("buckets") {
		data = append(data, map[string]interface{}{
			"count": bucket.Get("doc_count"),
			"key":   bucket.Get("key"),
		})

	}

	return data, nil
}

func (s *ReportService) ReportHistogram(interval string, options core.ReportOptions) (interface{}, error) {

	query := NewEventQuery()

	if options.AddressFilter != "" {
		query.ShouldHaveIp(options.AddressFilter, s.es.GetKeyword())
	}

	if options.QueryString != "" {
		query.AddFilter(QueryString(options.QueryString))
	}

	if options.TimeRange != "" {
		query.AddTimeRangeFilter(options.TimeRange)
	}

	if options.SensorFilter != "" {
		query.AddFilter(KeywordTermQuery("host", options.SensorFilter, s.es.GetKeyword()))
	}

	if options.EventType != "" {
		query.AddFilter(KeywordTermQuery("event_type", options.EventType, s.es.GetKeyword()))
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
	var data []map[string]interface{}
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

func (s *ReportService) ReportAggs(agg string, options core.ReportOptions) (interface{}, error) {

	size := int64(10)

	query := NewEventQuery()
	query.SetSize(0)

	// Event type...
	if options.EventType != "" {
		query.EventType(options.EventType)
	}

	// Narrow the type even further...
	if options.DnsType != "" {
		query.AddFilter(TermQuery("dns.type", options.DnsType))
	}

	if options.QueryString != "" {
		query.AddFilter(QueryString(options.QueryString))
	}

	if options.AddressFilter != "" {
		query.ShouldHaveIp(options.AddressFilter, s.es.GetKeyword())
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

	aggregations := map[string]string{
		// Generic.
		"src_ip":    "keyword",
		"dest_ip":   "keyword",
		"src_port":  "term",
		"dest_port": "term",

		// Alert.
		"alert.category":  "keyword",
		"alert.signature": "keyword",

		// DNS.
		"dns.rrname": "keyword",
		"dns.rrtype": "keyword",
		"dns.rcode":  "keyword",
		"dns.rdata":  "keyword",

		// SSH.
		"ssh.client.software_version": "keyword",
		"ssh.server.software_version": "keyword",

		// Generic.
		"traffic.id":    "keyword",
		"traffic.label": "keyword",
	}

	aggType := aggregations[agg]
	if aggType == "" {
		log.Warning("Unknown aggregation type for %s, will use term.", agg)
		aggType = "term"
	}

	if aggType == "keyword" {
		query.Aggs[agg] = map[string]interface{}{
			"terms": map[string]interface{}{
				"field": s.es.FormatKeyword(agg),
				"size":  size,
			},
		}
		query.Aggs["missing"] = map[string]interface{}{
			"missing": map[string]interface{}{
				"field": s.es.FormatKeyword(agg),
			},
		}
	} else {
		query.Aggs[agg] = map[string]interface{}{
			"terms": map[string]interface{}{
				"field": agg,
				"size":  size,
			},
		}
		query.Aggs["missing"] = map[string]interface{}{
			"missing": map[string]interface{}{
				"field": agg,
			},
		}
	}

	response, err := s.es.Search(query)
	if err != nil {
		return nil, err
	}

	if response.Error != nil {
		if response.Aggregations != nil {
			log.Warning("Elastic Search errors occurred, but will continue: %s",
				util.ToJson(response.Error))
		} else {
			return nil, errors.New(util.ToJson(response.Error))
		}
	}

	data := make([]map[string]interface{}, 0)
	missing := int64(0)
	other := int64(0)

	// Unwrap response.
	if response.Aggregations != nil {
		buckets := util.JsonMap(response.Aggregations[agg].(map[string]interface{})).GetMapList("buckets")
		for _, bucket := range buckets {
			data = append(data, map[string]interface{}{
				"key":   bucket["key"],
				"count": bucket["doc_count"],
			})
		}

		missing = response.Aggregations.GetMap("missing").GetInt64("doc_count")
		other = response.Aggregations.GetMap(agg).GetInt64("sum_other_doc_count")
	}

	return map[string]interface{}{
		"data":    data,
		"missing": missing,
		"other":   other,
	}, nil
}

package elasticsearch

import (
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/log"
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
		log.Println(bucket)

		data = append(data, map[string]interface{}{
			"count": bucket.Get("doc_count"),
			"key":   bucket.Get("key"),
		})

	}

	return data, nil
}

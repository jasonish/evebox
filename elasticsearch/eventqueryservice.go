package elasticsearch

import (
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/log"
)

type EventQueryService struct {
	es *ElasticSearch
}

func NewEventQueryService(es *ElasticSearch) *EventQueryService {
	return &EventQueryService{
		es: es,
	}
}

func (s *EventQueryService) Query(options core.EventQueryOptions) (interface{}, error) {

	query := NewEventQuery()
	query.MustNot(TermQuery("event_type", "stats"))
	query.SortBy("@timestamp", "desc")

	if options.Size > 0 {
		query.Size = options.Size
	} else {
		query.Size = 500
	}

	if options.QueryString != "" {
		query.AddFilter(QueryString(options.QueryString))
	}

	if options.MinTs != "" {
		query.AddFilter(RangeGte("timestamp", options.MinTs))
	}

	if options.MaxTs != "" {
		query.AddFilter(RangeLte("timestamp", options.MaxTs))
	}

	if options.EventType != "" {
		query.AddFilter(TermQuery("event_type", options.EventType))
	}

	log.Println(query)

	response, err := s.es.Search(query)
	if err != nil {
		log.Error("%v", err)
	}

	return response, nil
}

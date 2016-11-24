package postgres

import (
	"bytes"
	"fmt"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/elasticsearch"
	"github.com/jasonish/evebox/log"
	"github.com/satori/go.uuid"
	"gopkg.in/square/go-jose.v1/json"
)

type EventService struct {
	core.NotImplementedEventService

	pg *Service
}

func NewEventService(pg *Service) *EventService {
	service := EventService{}
	service.pg = pg
	return &service
}

func (s *EventService) FindNetflow(options core.EventQueryOptions, sortBy string, order string) (interface{}, error) {

	size := int64(10)

	sql := `SELECT
                  uuid,
                  source
                FROM events_master
                WHERE source ->> 'event_type' = 'netflow'
`

	if order == "" {
		order = "desc"
	}

	if options.Size > 0 {
		size = options.Size
	}

	if options.TimeRange != "" {
		sql += fmt.Sprintf(" AND timestamp >= NOW() - '%s'::interval", options.TimeRange)
	}

	if sortBy == "netflow.pkts" {
		sql += fmt.Sprintf(" ORDER BY source -> 'netflow' ->> 'pkts' %s", order)
	} else if sortBy == "netflow.bytes" {
		sql += fmt.Sprintf(" ORDER BY source -> 'netflow' ->> 'bytes' %s", order)
	} else {
		return nil, fmt.Errorf("Unknown sort field: %s", sortBy)
	}

	sql += fmt.Sprintf(" LIMIT %d", size)

	log.Println(sql)

	rows, err := s.pg.db.Query(sql)
	if err != nil {
		return nil, err
	}

	data := []map[string]interface{}{}

	for rows.Next() {
		var eventId uuid.UUID
		var rawSource []byte
		var source map[string]interface{}

		err = rows.Scan(&eventId, &rawSource)
		if err != nil {
			return nil, err
		}

		decoder := json.NewDecoder(bytes.NewReader(rawSource))
		decoder.UseNumber()
		err = decoder.Decode(&source)
		if err != nil {
			return nil, err
		}

		log.Println(elasticsearch.ToJson(source))

		data = append(data, map[string]interface{}{
			"_id":     eventId.String(),
			"_source": source,
		})
	}

	return map[string]interface{}{
		"data": data,
	}, nil
}

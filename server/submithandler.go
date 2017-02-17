package server

import (
	"encoding/json"
	"github.com/jasonish/evebox/elasticsearch"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/useragent"
	"io"
	"net/http"
)

type SubmitResponse struct {
	Count int
}

// Consumes events from agents and adds them to the database.
//
// TODO Refactor the actual event handling to a service that isn't
//     ElasticSearch specific, and will re-use the filters.
func SubmitHandler(appContext AppContext, r *http.Request) interface{} {

	count := 0

	decoder := json.NewDecoder(r.Body)
	decoder.UseNumber()

	es := appContext.ElasticSearch
	eventSink := elasticsearch.NewIndexer(es)

	geoFilter := eve.NewGeoipFilter(appContext.GeoIpService)
	tagsFilter := eve.TagsFilter{}
	uaFilter := useragent.EveUserAgentFilter{}

	for {
		var event map[string]interface{}

		err := decoder.Decode(&event)
		if err != nil {
			if err == io.EOF {
				break
			}
			log.Error("failed to decode incoming event: %v", err)
			return err
		}

		tagsFilter.Filter(event)
		geoFilter.Filter(event)
		uaFilter.Filter(event)

		eventSink.IndexRawEvent(event)

		count++
	}

	_, err := eventSink.FlushConnection()
	if err != nil {
		log.Error("Failed to commit events: %v", err)
		return err
	}

	log.Debug("Committed %d events from %v", count, r.RemoteAddr)

	return SubmitResponse{
		Count: count,
	}
}

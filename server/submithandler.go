package server

import (
	"encoding/json"
	"github.com/jasonish/evebox/elasticsearch"
	"github.com/jasonish/evebox/log"
	"io"
	"net/http"
)

type SubmitResponse struct {
	Count int
}

func SubmitHandler(appContext AppContext, r *http.Request) interface{} {

	count := 0

	decoder := json.NewDecoder(r.Body)
	decoder.UseNumber()

	es := appContext.ElasticSearch
	eventSink := elasticsearch.NewIndexer(es)

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

		eventSink.IndexRawEvent(event)

		count++
	}

	_, err := eventSink.FlushConnection()
	if err != nil {
		log.Error("Failed to commit events: %v", err)
		return err
	}

	log.Info("Committed %d events from %v", count, r.RemoteAddr)

	return SubmitResponse{
		Count: count,
	}
}

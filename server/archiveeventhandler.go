package server

import (
	"github.com/gorilla/mux"
	"github.com/jasonish/evebox/log"
	"net/http"
)

func ArchiveEventHandler(appContext AppContext, r *http.Request) interface{} {
	eventId := mux.Vars(r)["id"]

	err := appContext.EventService.AddTagsToEvent(eventId,
		[]string{"archived", "evebox.archived"})
	if err != nil {
		log.Error("%v", err)
		return err
	}

	return HttpOkResponse()
}

func EscalateEventHandler(appContext AppContext, r *http.Request) interface{} {
	eventId := mux.Vars(r)["id"]

	err := appContext.EventService.AddTagsToEvent(eventId,
		[]string{"escalated", "evebox.escalated"})
	if err != nil {
		log.Error("%v", err)
		return err
	}

	return HttpOkResponse()
}

func DeEscalateEventHandler(appContext AppContext, r *http.Request) interface{} {
	eventId := mux.Vars(r)["id"]

	err := appContext.EventService.RemoveTagsFromEvent(eventId,
		[]string{"escalated", "evebox.escalated"})
	if err != nil {
		log.Error("%v", err)
		return err
	}

	return HttpOkResponse()
}

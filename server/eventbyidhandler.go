package server

import (
	"fmt"
	"github.com/gorilla/mux"
	"github.com/jasonish/evebox/log"
	"net/http"
)

func GetEventByIdHandler(appContext AppContext, r *http.Request) interface{} {
	eventId := mux.Vars(r)["id"]
	event, err := appContext.EventService.GetEventById(eventId)
	if err != nil {
		log.Error("%v", err)
		return err
	}
	if event == nil {
		return HttpNotFoundResponse(fmt.Sprintf("No event with ID %s", eventId))
	}
	return event
}

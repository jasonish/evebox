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

package api

import (
	"fmt"
	"github.com/gorilla/mux"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/log"
	"net/http"
	"strconv"
)

func (c *ApiContext) GetEventByIdHandler(w *ResponseWriter, r *http.Request) error {
	eventId := mux.Vars(r)["id"]
	event, err := c.appContext.DataStore.GetEventById(eventId)
	if err != nil {
		log.Error("%v", err)
		return err
	}
	if event == nil {
		//return HttpNotFoundResponse(fmt.Sprintf("No event with ID %s", eventId))
		//return errors.New(fmt.Sprintf("No event with ID %s", eventId))
		return httpNotFoundResponse(fmt.Sprintf("No event with ID %s", eventId))
	}
	return w.OkJSON(event)
}

// Archive a single event.
func (c *ApiContext) ArchiveEventHandler(w *ResponseWriter, r *http.Request) error {
	eventId := mux.Vars(r)["id"]

	err := c.appContext.EventService.AddTagsToEvent(eventId,
		[]string{"archived", "evebox.archived"})
	if err != nil {
		log.Error("%v", err)
		return err
	}

	return w.Ok()
}

func (c *ApiContext) EscalateEventHandler(w *ResponseWriter, r *http.Request) error {
	eventId := mux.Vars(r)["id"]

	err := c.appContext.EventService.AddTagsToEvent(eventId,
		[]string{"escalated", "evebox.escalated"})
	if err != nil {
		log.Error("%v", err)
		return err
	}

	return w.Ok()
}

func (c *ApiContext) DeEscalateEventHandler(w *ResponseWriter, r *http.Request) error {
	eventId := mux.Vars(r)["id"]

	err := c.appContext.EventService.RemoveTagsFromEvent(eventId,
		[]string{"escalated", "evebox.escalated"})
	if err != nil {
		log.Error("%v", err)
		return err
	}

	return w.Ok()
}

func (c *ApiContext) EventQueryHandler(w *ResponseWriter, r *http.Request) error {

	var options core.EventQueryOptions

	options.QueryString = r.FormValue("queryString")
	options.MaxTs = r.FormValue("maxTs")
	options.MinTs = r.FormValue("minTs")
	options.EventType = r.FormValue("eventType")
	options.Size, _ = strconv.ParseInt(r.FormValue("size"), 0, 64)

	response, err := c.appContext.DataStore.EventQuery(options)
	if err != nil {
		return err
	}

	return w.OkJSON(response)
}

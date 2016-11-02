/* Copyright (c) 2014-2015 Jason Ish
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

package server

import (
	"encoding/json"
	"net/http"

	"fmt"
	"github.com/gorilla/mux"
	"github.com/jasonish/evebox/log"
)

type HttpStatusResponseBody struct {
	StatusCode int    `json:"status"`
	Message    string `json:"message,omitempty"`
}

type HttpResponse struct {
	statusCode  int
	contentType string
	body        interface{}
}

func HttpNotFoundResponse(message string) HttpResponse {
	return HttpResponse{
		statusCode:  http.StatusNotFound,
		contentType: "application/json",
		body: HttpStatusResponseBody{
			http.StatusNotFound,
			message,
		},
	}
}

func HttpOkResponse() HttpResponse {
	return HttpResponse{
		statusCode: http.StatusOK,
		body: HttpStatusResponseBody{
			StatusCode: http.StatusOK,
		},
	}
}

type ArchiveHandlerRequest struct {
	SignatureId  uint64 `json:"signature_id"`
	SrcIp        string `json:"src_ip"`
	DestIp       string `json:"dest_ip"`
	MinTimestamp string `json:"min_timestamp"`
	MaxTimestamp string `json:"max_timestamp"`
}

func ArchiveHandler(appContext AppContext, r *http.Request) interface{} {
	var request ArchiveHandlerRequest
	decoder := json.NewDecoder(r.Body)
	decoder.UseNumber()
	decoder.Decode(&request)
	err := appContext.ArchiveService.ArchiveAlerts(request.SignatureId, request.SrcIp,
		request.DestIp, request.MinTimestamp, request.MaxTimestamp)
	if err != nil {
		log.Error("%v", err)
		return err
	}
	return HttpOkResponse()
}

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

func ApiFunc(appContext AppContext, handler func(appContext AppContext, r *http.Request) interface{}) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		response := handler(appContext, r)
		if response != nil {
			switch response := response.(type) {
			case error:
				w.Header().Set("Content-Type", "application/json")
				w.WriteHeader(http.StatusBadRequest)
				encoder := json.NewEncoder(w)
				encoder.Encode(HttpStatusResponseBody{
					StatusCode: http.StatusBadRequest,
					Message:    response.Error(),
				})
			case HttpResponse:
				statusCode := http.StatusOK
				contentType := "application/json"
				if response.statusCode != 0 {
					statusCode = response.statusCode
				}
				if response.contentType != "" {
					contentType = response.contentType
				}
				w.Header().Set("Content-Type", contentType)
				w.WriteHeader(statusCode)
				if response.body != nil {
					encoder := json.NewEncoder(w)
					encoder.Encode(response.body)
				}
			default:
				w.Header().Set("Content-Type", "application/json")
				w.WriteHeader(http.StatusOK)
				encoder := json.NewEncoder(w)
				encoder.Encode(response)
			}
		}
	})
}

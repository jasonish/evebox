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

package server

import (
	"encoding/json"
	"net/http"

	"github.com/jasonish/evebox/elasticsearch"
	"github.com/jasonish/evebox/log"
	"github.com/pkg/errors"
)

type HttpStatusResponseBody struct {
	StatusCode int    `json:"status"`
	Message    string `json:"message,omitempty"`
}

// HttpResponse can be returned by API handlers to control how the response
// is processed.
type HttpResponse struct {
	// Set the status code of the reponse. If not provided, 200 (OK) will
	// be used.
	statusCode int

	// The content type of the response. Defaults to application/json as
	// this is primarily used for API responses.
	contentType string

	// Additional headers to set on the response.
	headers map[string]string

	// The body of the response. If content type is application/json or
	// empty (defaulting to application/json) the body will be serialized
	// to json.
	//
	// If the content type is no application/json it will be attempted
	// to be written out as bytes.
	body interface{}
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

type ApiHandler interface {
	ServeHTTP(appContext AppContext, r *http.Request) interface{}
}

type ApiHandlerFunc func(AppContext, *http.Request) interface{}

func (h ApiHandlerFunc) ServeHTTP(appContext AppContext, r *http.Request) interface{} {
	return h(appContext, r)
}

type ApiWrapper struct {
	appContext AppContext
	//handler    func(appContent AppContext, r *http.Request) interface{}
	handler ApiHandler
}

func (h ApiWrapper) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	response := h.handler.ServeHTTP(h.appContext, r)
	if response != nil {
		switch response := response.(type) {
		case error:
			var message string

			switch cause := errors.Cause(response).(type) {
			case *elasticsearch.DatastoreError:
				message = cause.Message
			}

			if message == "" {
				message = response.Error()
			}

			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusBadRequest)
			encoder := json.NewEncoder(w)
			encoder.Encode(HttpStatusResponseBody{
				StatusCode: http.StatusBadRequest,
				Message:    message,
			})
		case []byte:
			// Pass on the raw data.
			w.Write(response)
		case HttpResponse:
			statusCode := http.StatusOK
			contentType := "application/json"

			// Set status code if provided.
			if response.statusCode != 0 {
				statusCode = response.statusCode
			}

			// Set content type if provided.
			if response.contentType != "" {
				contentType = response.contentType
			}

			// Merge in provided headers.
			if response.headers != nil {
				for key, val := range response.headers {
					log.Info("Setting %s -> %s", key, val)
					w.Header().Set(key, val)
				}
			}

			w.Header().Set("Content-Type", contentType)
			w.WriteHeader(statusCode)
			if response.body != nil {
				switch body := response.body.(type) {
				case []byte:
					w.Write(body)
				default:
					encoder := json.NewEncoder(w)
					encoder.Encode(response.body)
				}
			}
		default:
			w.Header().Set("Content-Type", "application/json")
			w.WriteHeader(http.StatusOK)
			encoder := json.NewEncoder(w)
			encoder.Encode(response)
		}
	}
}

func ApiH(appContext AppContext, handler ApiHandler) http.Handler {
	return ApiWrapper{
		appContext,
		handler,
	}
}

func ApiF(appContext AppContext, handler func(AppContext, *http.Request) interface{}) http.Handler {
	return ApiH(appContext, ApiHandlerFunc(handler))
}

func DecodeRequestBody(r *http.Request, value interface{}) error {
	decoder := json.NewDecoder(r.Body)
	decoder.UseNumber()
	return decoder.Decode(value)
}

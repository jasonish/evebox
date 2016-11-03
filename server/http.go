package server

import (
	"encoding/json"
	"github.com/jasonish/evebox/log"
	"net/http"
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

type ApiHandler struct {
	appContext AppContext
	handler    func(appContent AppContext, r *http.Request) interface{}
}

func (h ApiHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	response := h.handler(h.appContext, r)
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
				if contentType == "application/json" {
					encoder := json.NewEncoder(w)
					encoder.Encode(response.body)
				} else {
					switch body := response.body.(type) {
					case []byte:
						w.Write(body)
					default:
						log.Error("Don't know how to write reponse body for content type %s", contentType)
					}
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

func ApiF(appContext AppContext, handler func(AppContext, *http.Request) interface{}) http.Handler {
	return ApiHandler{
		appContext,
		handler,
	}
}

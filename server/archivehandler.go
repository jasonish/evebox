package server

import (
	"encoding/json"
	"net/http"
	"time"

	"github.com/jasonish/evebox/elasticsearch"
	"github.com/jasonish/evebox/log"
)

type ArchiveHandlerRequest struct {
	SignatureId  uint64 `json:"signature_id"`
	SrcIp        string `json:"src_ip"`
	DestIp       string `json:"dest_ip"`
	MinTimestamp string `json:"min_timestamp"`
	MaxTimestamp string `json:"max_timestamp"`
}

var OkResponse map[string]string

func init() {
	OkResponse = map[string]string{
		"status": "ok",
	}
}

type ApiHandlerFunc func(w http.ResponseWriter, r *http.Request) error

type ApiHandler interface {
	ServeHTTP(w http.ResponseWriter, r *http.Request) (interface{}, error)
}

type ArchiveHandler struct {
	AppContext AppContext
}

func (h *ArchiveHandler) ServeHTTP(w http.ResponseWriter, r *http.Request) (interface{}, error) {
	var request ArchiveHandlerRequest
	decoder := json.NewDecoder(r.Body)
	decoder.UseNumber()
	decoder.Decode(&request)
	err := elasticsearch.ArchiveAlerts(h.AppContext.ElasticSearch,
		request.SignatureId, request.SrcIp,
		request.DestIp, request.MinTimestamp, request.MaxTimestamp)
	if err != nil {
		log.Error("%v", err)
		return nil, err
	}
	return OkResponse, nil
}

type ApiResponseWriter struct {
	w     http.ResponseWriter
	clean bool
}

func NewApiResponseWriter(w http.ResponseWriter) *ApiResponseWriter {
	return &ApiResponseWriter{
		w:     w,
		clean: true,
	}
}

func (w *ApiResponseWriter) Write(bytes []byte) (int, error) {
	w.clean = false
	return w.w.Write(bytes)
}

func (w *ApiResponseWriter) Header() http.Header {
	return w.w.Header()
}

func (w *ApiResponseWriter) WriteHeader(header int) {
	w.clean = false
	w.w.WriteHeader(header)
}

func Api(appContext AppContext, handler ApiHandler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		start := time.Now()
		responseWriterWrapper := NewApiResponseWriter(w)
		response, err := handler.ServeHTTP(responseWriterWrapper, r)
		if responseWriterWrapper.clean {
			if err != nil {
				w.Header().Set("Content-Type", "application/json")
				w.WriteHeader(http.StatusBadRequest)
				w.Write([]byte(err.Error()))
			} else {
				w.Header().Set("Content-Type", "application/json")
				w.WriteHeader(http.StatusOK)
				encoder := json.NewEncoder(w)
				encoder.Encode(response)
			}
		} else {
			log.Info("Response writer is not clean.")
		}
		duration := time.Since(start)
		log.Info("%s - %.6fs", r.URL, duration.Seconds())
	})
}

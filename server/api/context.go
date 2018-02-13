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
	"encoding/json"
	"github.com/jasonish/evebox/appcontext"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/server/auth"
	"github.com/jasonish/evebox/server/router"
	"github.com/jasonish/evebox/server/sessions"
	"github.com/pkg/errors"
	"net/http"
)

type ApiError struct {
	Status  int    `json:"status"`
	Message string `json:"message"`
}

func (e ApiError) Error() string {
	return e.Message
}

type httpErrorResponse struct {
	error
	status int
}

func (r *httpErrorResponse) MarshalJSON() ([]byte, error) {
	return json.Marshal(map[string]interface{}{
		"status": r.status,
		"error": map[string]interface{}{
			"message": r.Error(),
		},
	})
}

func httpNotFoundResponse(message string) *httpErrorResponse {
	return &httpErrorResponse{
		error:  errors.New(message),
		status: http.StatusNotFound,
	}
}

func newHttpErrorResponse(statusCode int, err error) *httpErrorResponse {
	return &httpErrorResponse{
		error:  err,
		status: statusCode,
	}
}

type apiHandlerFunc func(w *ResponseWriter, r *http.Request) error

func apiFuncWrapper(handler apiHandlerFunc) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		err := handler(NewResponseWriter(w), r)
		if err == nil {
			return
		}

		w.Header().Set("content-type", "application/json")
		encoder := json.NewEncoder(w)
		status := http.StatusInternalServerError

		switch err.(type) {
		case *core.EventNotFoundError:
			status = http.StatusNotFound
		}

		switch err := err.(type) {
		case ApiError:
			w.WriteHeader(err.Status)
			encoder.Encode(err)
		case *httpErrorResponse:
			w.WriteHeader(err.status)
			encoder.Encode(err)
		default:
			w.WriteHeader(http.StatusInternalServerError)
			encoder.Encode(&httpErrorResponse{
				error:  err,
				status: status,
			})
		}
	})
}

// apiRouter wraps the provided router with some helper functions for
// registering API handlers of type apiHandlerFunc.
type apiRouter struct {
	router *router.Router
}

func (r *apiRouter) GET(path string, handler apiHandlerFunc) {
	r.router.GET(path, apiFuncWrapper(handler))
}

func (r *apiRouter) POST(path string, handler apiHandlerFunc) {
	r.router.POST(path, apiFuncWrapper(handler))
}

func (r *apiRouter) OPTIONS(path string, handler apiHandlerFunc) {
	r.router.OPTIONS(path, apiFuncWrapper(handler))
}

type ApiContext struct {
	appContext    *appcontext.AppContext
	sessionStore  *sessions.SessionStore
	authenticator auth.Authenticator
}

func NewApiContext(appContext *appcontext.AppContext,
	sessionStore *sessions.SessionStore, authenticator auth.Authenticator) *ApiContext {
	return &ApiContext{
		appContext:    appContext,
		sessionStore:  sessionStore,
		authenticator: authenticator,
	}
}

func (c *ApiContext) InitRoutes(router *router.Router) {
	r := apiRouter{router}

	r.POST("/login", c.LoginHandler)
	r.OPTIONS("/login", c.LoginOptions)
	r.GET("/logout", c.LogoutHandler)

	r.GET("/alerts", c.AlertsHandler)
	r.POST("/alert-group/archive", c.AlertGroupArchiveHandler)
	r.POST("/alert-group/star", c.EscalateAlertGroupHandler)
	r.POST("/alert-group/unstar", c.DeEscalateAlertGroupHandler)
	r.POST("/alert-group/comment", c.CommentOnAlertGroupHandler)

	r.GET("/version", c.VersionHandler)
	r.POST("/submit", c.SubmitHandler)
	r.POST("/eve2pcap", c.Eve2PcapHandler)
	r.POST("/query", c.QueryHandler)
	r.GET("/config", c.ConfigHandler)
	r.POST("/event/{id}/archive", c.ArchiveEventHandler)
	r.POST("/event/{id}/escalate", c.EscalateEventHandler)
	r.POST("/event/{id}/de-escalate", c.DeEscalateEventHandler)
	r.POST("/event/{id}/comment", c.CommentOnEventHandler)
	r.GET("/event/{id}", c.GetEventByIdHandler)
	r.GET("/event-query", c.EventQueryHandler)
	r.GET("/report/dns/requests/rrnames", c.ReportDnsRequestRrnames)
	r.POST("/report/dns/requests/rrnames", c.ReportDnsRequestRrnames)
	r.GET("/netflow", c.NetflowHandler)
	r.GET("/report/agg", c.ReportAggs)
	r.GET("/report/histogram", c.ReportHistogram)
	r.POST("/find-flow", c.FindFlowHandler)

	r.GET("/flow/histogram", c.FlowHistogram)
}

// DecodeRequestBody is a helper functio to decoder request bodies into a
// particular interface.
func DecodeRequestBody(r *http.Request, value interface{}) error {
	decoder := json.NewDecoder(r.Body)
	decoder.UseNumber()
	return decoder.Decode(value)
}

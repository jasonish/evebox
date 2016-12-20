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
	"net/http"

	"github.com/gorilla/mux"
	"github.com/jasonish/evebox/log"
)

type Router struct {
	router *mux.Router
}

func NewRouter() *Router {
	return &Router{
		router: mux.NewRouter(),
	}
}

func (r *Router) Handle(path string, handler http.Handler) *mux.Route {
	return r.router.Handle(path, handler)
}

func (r *Router) Prefix(path string, handler http.Handler) {
	r.router.PathPrefix(path).Handler(handler)
}

func (r *Router) GET(path string, handler http.Handler) {
	log.Debug("Adding GET route: %s", path)
	r.router.Handle(path, handler).Methods("GET")
}

func (r *Router) POST(path string, handler http.Handler) {
	r.router.Handle(path, handler).Methods("POST")
}

type ApiRouter struct {
	appContext AppContext
	router     *Router
}

func (r *ApiRouter) Handle(path string, handler ApiHandlerFunc) {
	r.router.Handle(path, ApiF(r.appContext, handler))
}

func (r *ApiRouter) GET(path string, handler ApiHandlerFunc) {
	r.router.GET(path, ApiF(r.appContext, handler))
}

func (r *ApiRouter) POST(path string, handler ApiHandlerFunc) {
	r.router.POST(path, ApiF(r.appContext, handler))
}

type Server struct {
	appContext AppContext
	router     *Router
}

func NewServer(appContext AppContext) *Server {

	router := NewRouter()

	server := &Server{
		appContext: appContext,
		router:     router,
	}

	server.RegisterApiHandlers()

	return server
}

func (s *Server) Start(addr string) error {
	log.Printf("Listening on %s", addr)
	return http.ListenAndServe(addr, s.router.router)
}

func (s *Server) RegisterApiHandlers() {

	apiRouter := ApiRouter{s.appContext, s.router}

	apiRouter.Handle("/api/1/archive", ArchiveHandler)
	apiRouter.Handle("/api/1/escalate", EscalateHandler)

	apiRouter.POST("/api/1/alert-group/add-tags", AlertGroupAddTags)
	apiRouter.POST("/api/1/alert-group/remove-tags", AlertGroupRemoveTags)

	apiRouter.Handle("/api/1/event/{id}", GetEventByIdHandler)

	apiRouter.POST("/api/1/event/{id}/archive", ArchiveEventHandler)
	apiRouter.POST("/api/1/event/{id}/escalate", EscalateEventHandler)
	apiRouter.POST("/api/1/event/{id}/de-escalate", DeEscalateEventHandler)

	apiRouter.Handle("/api/1/config", ConfigHandler)
	apiRouter.Handle("/api/1/version", VersionHandler)
	apiRouter.Handle("/api/1/eve2pcap", Eve2PcapHandler)

	apiRouter.GET("/api/1/alerts", AlertsHandler)
	apiRouter.GET("/api/1/event-query", EventQueryHandler)

	apiRouter.Handle("/api/1/query", QueryHandler)

	apiRouter.Handle("/api/1/_bulk", EsBulkHandler)

	apiRouter.GET("/api/1/report/dns/requests/rrnames", ReportDnsRequestRrnames)
	apiRouter.POST("/api/1/report/dns/requests/rrnames", ReportDnsRequestRrnames)

	apiRouter.GET("/api/1/netflow", NetflowHandler)

	apiRouter.GET("/api/1/report/agg", ReportAggs)
	apiRouter.GET("/api/1/report/histogram", ReportHistogram)

	apiRouter.POST("/api/1/submit", SubmitHandler)

	// Static file server, must be last as it serves as the fallback.
	s.router.Prefix("/", StaticHandlerFactory(s.appContext))

}

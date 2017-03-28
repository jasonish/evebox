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

	"github.com/gorilla/handlers"
	"github.com/jasonish/evebox/appcontext"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/resources"
	"github.com/jasonish/evebox/server/api"
	"github.com/jasonish/evebox/server/router"
	"net/http/httputil"
	"net/url"
)

type Server struct {
	router *router.Router
}

func NewServer(appContext appcontext.AppContext) *Server {

	router := router.NewRouter()

	server := &Server{
		router: router,
	}

	apiContext := api.NewApiContext(&appContext)
	apiContext.InitRoutes(router.Subrouter("/api/1"))

	// Static file server, must be last as it serves as the fallback.
	router.Prefix("/", StaticHandlerFactory(appContext))

	return server
}

func (s *Server) Start(addr string) error {
	log.Info("Listening on %s", addr)
	return http.ListenAndServe(addr,
		handlers.CompressHandler(
			VersionHeaderWrapper(s.router.Router)))
}

func StaticHandlerFactory(appContext appcontext.AppContext) http.Handler {
	if appContext.Vars.DevWebAppServerUrl != "" {
		log.Notice("Proxying static files to %v.",
			appContext.Vars.DevWebAppServerUrl)
		devServerProxyUrl, err := url.Parse(appContext.Vars.DevWebAppServerUrl)
		if err != nil {
			log.Fatal(err)
		}
		return httputil.NewSingleHostReverseProxy(devServerProxyUrl)
	}
	return resources.FileServer{}
}

func VersionHeaderWrapper(handler http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("X-EveBox-Git-Revision", core.BuildRev)
		handler.ServeHTTP(w, r)
	})
}

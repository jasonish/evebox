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

	"context"
	"github.com/gorilla/handlers"
	"github.com/jasonish/evebox/appcontext"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/resources"
	"github.com/jasonish/evebox/server/api"
	"github.com/jasonish/evebox/server/auth"
	"github.com/jasonish/evebox/server/router"
	"github.com/jasonish/evebox/server/sessions"
	"github.com/spf13/viper"
	"net/http/httputil"
	"net/url"
	"strings"
)

var sessionStore = sessions.NewSessionStore()

func isPublic(r *http.Request) bool {
	path := r.URL.Path

	prefixes := []string{
		"/login",
		"/auth",
		"/public",
		"/api/1/login",
		"/api/1/logout",
		"/favicon.ico",
	}

	for _, prefix := range prefixes {
		if strings.HasPrefix(path, prefix) {
			return true
		}
	}

	return false
}

func Redirector(handler http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch r.URL.Path {
		case "/":
			r.URL.Path = "/public/index.html"
		case "/index.html":
			r.URL.Path = "/public/index.html"
		case "/favicon.ico":
			r.URL.Path = "/public/favicon.ico"
		}
		handler.ServeHTTP(w, r)
	})
}

var authenticator auth.Authenticator

func SessionHandler(handler http.Handler) http.Handler {

	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {

		// If public, pass through.
		if isPublic(r) {
			handler.ServeHTTP(w, r)
			return
		}

		session := authenticator.Authenticate(w, r)
		if session != nil {
			context := context.WithValue(r.Context(),
				"session", session)
			handler.ServeHTTP(w, r.WithContext(context))
		}
	})
}

type Server struct {
	router *router.Router
}

func NewServer(appContext appcontext.AppContext) *Server {

	authenticationRequired := viper.GetBool("authentication.required")
	if authenticationRequired {
		authenticationType := viper.GetString("authentication.type")
		switch authenticationType {
		case "":
			log.Fatal("Authentication requested but no type set.")
		case "username":
			authenticator = auth.NewUsernameAuthenticator(sessionStore)
		default:
			log.Fatalf("Unsupported authentication type: %s",
				authenticationType)
		}
	} else {
		log.Info("Authentication disabled.")
		authenticator = auth.NewAnonymousAuthenticator(sessionStore)
	}

	router := router.NewRouter()

	server := &Server{
		router: router,
	}

	githubAuth := auth.NewGithub()

	router.Handle("/auth/github", http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		githubAuth.Handler(w, r)
	}))

	router.Handle("/auth/github/callback", http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		githubAuth.Callback(w, r)
	}))

	apiContext := api.NewApiContext(&appContext, sessionStore, authenticator)
	apiContext.InitRoutes(router.Subrouter("/api/1"))

	// Static file server, must be last as it serves as the fallback.
	router.Prefix("/public", StaticHandlerFactory(appContext))

	return server
}

func (s *Server) Start(addr string) error {
	log.Info("Listening on %s", addr)
	return http.ListenAndServe(addr,
		handlers.CompressHandler(
			Redirector(
				VersionHeaderWrapper(
					SessionHandler(s.router.Router)))))
}

func StaticHandlerFactory(appContext appcontext.AppContext) http.Handler {
	if appContext.Vars.DevWebAppServerUrl != "" {
		log.Notice("Proxying static files to %v.",
			appContext.Vars.DevWebAppServerUrl)
		devServerProxyUrl, err := url.Parse(appContext.Vars.DevWebAppServerUrl)
		if err != nil {
			log.Fatal(err)
		}

		proxy := httputil.NewSingleHostReverseProxy(devServerProxyUrl)
		return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			stripped := strings.TrimPrefix(r.URL.Path, "/public")
			log.Debug("Proxying %s -> %s", r.URL.Path, stripped)
			r.URL.Path = stripped
			proxy.ServeHTTP(w, r)
		})
	}
	return resources.FileServer{}
}

func VersionHeaderWrapper(handler http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("X-EveBox-Git-Revision", core.BuildRev)
		handler.ServeHTTP(w, r)
	})
}

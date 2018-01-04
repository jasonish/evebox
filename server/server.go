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
	"crypto/tls"
	"fmt"
	"github.com/gorilla/handlers"
	"github.com/jasonish/evebox/appcontext"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/server/api"
	"github.com/jasonish/evebox/server/auth"
	"github.com/jasonish/evebox/server/router"
	"github.com/jasonish/evebox/server/sessions"
	"github.com/spf13/viper"
	"golang.org/x/crypto/acme/autocert"
	"path"
	"strings"
	"time"
	"github.com/jasonish/evebox/resources"
)

const DEFAULT_PORT = 5636

const SESSION_HEADER = "x-evebox-session-id"

var sessionStore = sessions.NewSessionStore()

func isPublic(r *http.Request) bool {
	path := r.URL.Path

	prefixes := []string{
		"/auth",
		"/public",
		"/api/1/version",
		"/api/1/login",
		"/api/1/logout",
		"/favicon.ico",

		// Agent's do not require authentication at this time.
		"/api/1/submit",
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
			r.URL.Path = "/public/"
		case "/index.html":
			r.URL.Path = "/public/"
		case "/favicon.ico":
			r.URL.Path = "/public/favicon.ico"
		}
		handler.ServeHTTP(w, r)
	})
}

var authenticator auth.Authenticator

func SessionHandler(handler http.Handler) http.Handler {

	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {

		ctx := r.Context()
		session := sessionStore.FindSession(r)
		if session != nil {
			ctx = context.WithValue(ctx, "session", session)
		}

		// If public, pass through.
		if isPublic(r) {
			handler.ServeHTTP(w, r.WithContext(ctx))
			return
		}

		session = authenticator.Authenticate(w, r)
		if session != nil {
			context := context.WithValue(r.Context(),
				"session", session)
			handler.ServeHTTP(w, r.WithContext(context))
		}
	})
}

func sessionReaper(sessionStore *sessions.SessionStore) {
	ticker := time.NewTicker(60 * time.Second)
	go func() {
		for {
			<-ticker.C
			log.Debug("Reaping sessions.")
			sessionStore.Reap()
		}
	}()
	log.Info("Session reaper started")
}

type Server struct {
	router  *router.Router
	proxied bool
	context appcontext.AppContext
}

func NewServer(appContext appcontext.AppContext) *Server {

	sessionReaper(sessionStore)

	sessionStore.Header = SESSION_HEADER

	authRequired := appContext.Config.Authentication.Required
	if authRequired {
		authenticationType := appContext.Config.Authentication.Type
		switch authenticationType {
		case "":
			log.Fatal("Authentication requested but no type set.")
		case "username":
			authenticator = auth.NewUsernameAuthenticator(sessionStore)
		case "usernamepassword":
			if appContext.ConfigDB.InMemory {
				log.Fatal("Username/password authentication not supported with in-memory configuration database.")
			}
			authenticator = auth.NewUsernamePasswordAuthenticator(sessionStore,
				appContext.Userstore)
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
		router:  router,
		proxied: appContext.Config.Http.ReverseProxy,
		context: appContext,
	}

	if appContext.Config.Authentication.Github.Enabled {
		githubAuthenticator := auth.NewGithub(
			appContext.Config.Authentication.Github,
			appContext.Userstore)
		githubAuthenticator.SessionStore = sessionStore

		router.Handle("/auth/github", http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			githubAuthenticator.Handler(w, r)
		}))

		router.Handle("/auth/github/callback", http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
			githubAuthenticator.Callback(w, r)
		}))
		log.Info("GitHub Oauth2 authentication configured")
	}

	apiContext := api.NewApiContext(&appContext, sessionStore, authenticator)
	apiContext.InitRoutes(router.Subrouter("/api/1"))

	// Static file server, must be last as it serves as the fallback.
	router.Prefix("/public", http.FileServer(resources.ResourceBox))

	return server
}

type RequestLogWrapper struct {
}

func (w RequestLogWrapper) Write(b []byte) (n int, err error) {
	log.Info("HTTP: %s", strings.TrimSpace(string(b)))
	return len(b), nil
}

func (s *Server) setupHandlers() http.Handler {
	root := http.Handler(s.router.Router)
	root = SessionHandler(root)
	root = VersionHeaderWrapper(root)
	root = Redirector(root)

	if s.context.Config.Http.RequestLogging {
		log.Debug("Enabling HTTP request logging")
		root = handlers.LoggingHandler(RequestLogWrapper{}, root)
	}

	if s.proxied {
		log.Debug("Apply reverse proxy handler")
		root = handlers.ProxyHeaders(root)
	}

	root = handlers.CompressHandler(root)

	return root
}

func (s *Server) startWithAutoCert(host string, port uint16) error {

	if port != 443 {
		log.Warning("Server not running on port 443; Letsencrypt may not work.")
	}

	dataDirectory := viper.GetString("data-directory")
	if dataDirectory == "" {
		log.Fatal("A data directory must be configured to use Lets Encrypt!")
	}

	cacheDir := path.Join(dataDirectory, "certs")

	certManager := autocert.Manager{
		Prompt:     autocert.AcceptTOS,
		HostPolicy: autocert.HostWhitelist(s.context.Config.LetsEncryptHostname),
		Cache:      autocert.DirCache(cacheDir),
	}

	server := &http.Server{
		Addr: fmt.Sprintf("%s:%d", host, port),
		TLSConfig: &tls.Config{
			GetCertificate: certManager.GetCertificate,
		},
		Handler: s.setupHandlers(),
	}

	log.Info("Starting server on %s:%d with Letsencrypt support for hostname %s",
		host, port, s.context.Config.LetsEncryptHostname)

	return server.ListenAndServeTLS("", "")
}

func (s *Server) Start(host string, port uint16) error {

	config := s.context.Config

	if config.LetsEncryptHostname != "" {
		return s.startWithAutoCert(host, port)
	}

	root := s.setupHandlers()
	listenAddr := fmt.Sprintf("%s:%d", host, port)

	if !config.Http.TlsEnabled {
		log.Info("Listening on %s", listenAddr)
		return http.ListenAndServe(listenAddr, root)
	} else {
		log.Info("Listening with TLS on %s", listenAddr)
		if config.Http.TlsCertificate == "" {
			log.Fatalf("TLS requested but certificate file not provided.")
		}
		keyFile := config.Http.TlsKey
		if keyFile == "" {
			keyFile = config.Http.TlsCertificate
		}
		return http.ListenAndServeTLS(listenAddr,
			config.Http.TlsCertificate,
			keyFile, root)
	}
}

func VersionHeaderWrapper(handler http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("X-EveBox-Git-Revision", core.BuildRev)
		handler.ServeHTTP(w, r)
	})
}

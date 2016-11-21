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
	"github.com/gorilla/mux"
	"net/http"
)

type Router struct {
	router *mux.Router
}

func NewRouter() *Router {
	return &Router{
		router: mux.NewRouter(),
	}
}

func (r *Router) Handle(path string, handler http.Handler) {
	r.router.Handle(path, handler)
}

func (r *Router) Prefix(path string, handler http.Handler) {
	r.router.PathPrefix(path).Handler(handler)
}

func (r *Router) GET(path string, handler http.Handler) {
	r.router.Handle(path, handler).Methods("GET")
}

func (r *Router) POST(path string, handler http.Handler) {
	r.router.Handle(path, handler).Methods("POST")
}

type Server struct {
	router *Router
}

func NewServer(router *Router) *Server {
	return &Server{
		router: router,
	}
}

func (s *Server) Start(addr string) error {
	return http.ListenAndServe(addr, s.router.router)
}

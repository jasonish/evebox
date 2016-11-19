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

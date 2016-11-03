package http

import "net/http"

// Chain sets up a chain of handlers (middleware) for re-use.
type Chain struct {
	handlers []func(http.Handler) http.Handler
}

func (c *Chain) Add(handler func(http.Handler) http.Handler) {
	c.handlers = append(c.handlers, handler)
}

func (c *Chain) Handle(handler http.Handler) http.Handler {
	handlerCount := len(c.handlers)

	for handlerCount > 0 {
		handler = c.handlers[handlerCount-1](handler)
		handlerCount--
	}

	return handler
}

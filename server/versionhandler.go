package server

import (
	"github.com/jasonish/evebox/core"
	"net/http"
)

type VersionResponse struct {
	Version  string `json:"version"`
	Revision string `json:"revision"`
	Date     string `json:"date"`
}

func VersionHandler(appContext AppContext, r *http.Request) interface{} {
	response := VersionResponse{
		core.BuildVersion,
		core.BuildRev,
		core.BuildDate,
	}
	return response
}

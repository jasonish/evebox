package server

import (
	"net/http"

	"github.com/jasonish/evebox/core"
)

func NetflowHandler(appContext AppContext, r *http.Request) interface{} {

	options := core.EventQueryOptionsFromHttpRequest(r)

	sortBy := r.FormValue("sortBy")

	response, err := appContext.EventService.FindNetflow(options, sortBy, "")
	if err != nil {
		return err
	}
	return response
}

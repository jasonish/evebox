package server

import (
	"github.com/jasonish/evebox/core"
	"net/http"
)

func EventQueryHandler(appContext AppContext, r *http.Request) interface{} {

	var options core.EventQueryOptions

	options.QueryString = r.FormValue("queryString")
	options.MaxTs = r.FormValue("maxTs")
	options.MinTs = r.FormValue("minTs")
	options.EventType = r.FormValue("eventType")

	response, err := appContext.EventQueryService.Query(options)
	if err != nil {
		return err
	}

	return response
}

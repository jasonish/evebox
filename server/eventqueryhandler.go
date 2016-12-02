package server

import (
	"github.com/jasonish/evebox/core"
	"net/http"
	"strconv"
)

func EventQueryHandler(appContext AppContext, r *http.Request) interface{} {

	var options core.EventQueryOptions

	options.QueryString = r.FormValue("queryString")
	options.MaxTs = r.FormValue("maxTs")
	options.MinTs = r.FormValue("minTs")
	options.EventType = r.FormValue("eventType")
	options.Size, _ = strconv.ParseInt(r.FormValue("size"), 0, 64)

	response, err := appContext.DataStore.EventQuery(options)
	//response, err := appContext.EventQueryService.EventQuery(options)
	if err != nil {
		return err
	}

	return response
}

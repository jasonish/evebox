package server

import (
	"github.com/jasonish/evebox/core"
	"net/http"
	"strconv"
)

func ReportDnsRequestRrnames(appContext AppContext, r *http.Request) interface{} {

	options := core.ReportOptions{}

	if r.Method == http.MethodPost {
		var requestBody struct {
			TimeRange string `json:"timeRange"`
			Size      int64  `json:"size"`
		}
		DecodeRequestBody(r, &requestBody)
		options.TimeRange = requestBody.TimeRange
		options.Size = requestBody.Size
	} else {
		if r.FormValue("timeRange") != "" {
			options.TimeRange = r.FormValue("timeRange")
		}
		if r.FormValue("size") != "" {
			options.Size, _ = strconv.ParseInt(r.FormValue("size"), 10, 64)
		}
	}

	data, err := appContext.ReportService.ReportDnsRequestRrnames(options)
	if err != nil {
		return err
	}

	return map[string]interface{}{
		"data": data,
	}

}

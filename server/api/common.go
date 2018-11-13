package api

import (
	"encoding/json"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"net/http"
	"strconv"
	"time"
)

type CommonRequestArgs struct {
	MinTs       time.Time
	MaxTs       time.Time
	TimeRange   string
	QueryString string
	Size        int64
	EventType   string
}

func parseCommonRequestArgs(r *http.Request) (CommonRequestArgs, error) {
	var err error = nil

	args := CommonRequestArgs{}

	args.EventType = r.FormValue("event_type")

	if r.FormValue("size") != "" {
		args.Size, err = strconv.ParseInt(r.FormValue("size"), 10, 64)
		if err != nil {
			return args, nil
		}
	}

	// time_range with timeRange fallback.
	args.TimeRange = r.FormValue("time_range")
	if args.TimeRange == "" {
		args.TimeRange = r.FormValue("timeRange")
		if args.TimeRange != "" {
			log.Warning("Found deprecated query string parameter 'timeRange'.")
		}
	}

	minTs := r.FormValue("min_ts")
	if minTs != "" {
		args.MinTs, err = eve.ParseTimestamp(minTs)
		if err != nil {
			return args, err
		}
	}

	maxTs := r.FormValue("max_ts")
	if maxTs != "" {
		args.MaxTs, err = eve.ParseTimestamp(maxTs)
		if err != nil {
			return args, err
		}
	}

	// query_string will queryString fallback.
	args.QueryString = r.FormValue("query_string")
	if args.QueryString == "" {
		args.QueryString = r.FormValue("queryString")
		if args.QueryString != "" {
			log.Warning("Found deprecated query string parameter 'queryString'.")
		}
	}

	return args, nil
}

// DecodeRequestBody is a helper function to decoder request bodies into a
// particular interface.
func DecodeRequestBody(r *http.Request, value interface{}) error {
	decoder := json.NewDecoder(r.Body)
	decoder.UseNumber()
	return decoder.Decode(value)
}

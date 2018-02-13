/* Copyright (c) 2018 Jason Ish
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

package api

import (
	"net/http"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/eve"
	"time"
	"fmt"
	"strings"
)

type CommonRequestArgs struct {
	MinTs       time.Time
	MaxTs       time.Time
	TimeRange   string
	QueryString string
}

func parseCommonRequestArgs(r *http.Request) (CommonRequestArgs, error) {
	var err error = nil

	args := CommonRequestArgs{}

	args.TimeRange = r.FormValue("time_range")

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

	args.QueryString = r.FormValue("query_string")

	return args, nil
}

// Parameters:
//
//   sub_aggs: comma separated list of sub-aggregations, values include:
//               - app_proto
func (c *ApiContext) FlowHistogram(w *ResponseWriter, r *http.Request) error {
	if c.appContext.FlowService == nil {
		log.Warning("Flow service not implemented.")
		w.WriteHeader(http.StatusNotImplemented)
		return nil
	}

	args, err := parseCommonRequestArgs(r)
	if err != nil {
		return err
	}

	if args.TimeRange != "" && !(args.MinTs.IsZero() && args.MaxTs.IsZero()) {
		return fmt.Errorf("time_range not allowed with min_ts or max_ts")
	}

	interval := r.FormValue("interval")

	options := core.FlowHistogramOptions{
		MinTs:       args.MinTs,
		MaxTs:       args.MaxTs,
		TimeRange:   args.TimeRange,
		Interval:    interval,
		SubAggs:     strings.Split(r.FormValue("sub_aggs"), ","),
		QueryString: args.QueryString,
	}

	response := c.appContext.FlowService.Histogram(options)
	w.OkJSON(response)
	return nil
}

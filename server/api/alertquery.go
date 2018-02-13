/* Copyright (c) 2016-2017 Jason Ish
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
	"github.com/jasonish/evebox/core"
	"net/http"
	"strings"
	"fmt"
)

// AlertsHandler handles GET requests to /api/1/alerts. This is the handler
// for the Inbox, Escalated and Alerts view queries.
//
// Accepted query parameters:
//
//     tags: a list of tags alerts must have, or must not have; must have tags
//         are prefixed with a "-".
//
//     query_string: a query string alerts must match, exact format depends
//         on the database used.
//
//     time_range: a duration strings (ie: 60s) representing the time before now,
//         until now that alerts must match.
//
//     min_ts: specify the earliest timestamp for the range of the query,
//         format: YYYY-MM-DDTHH:MM:SS.UUUUUUZ
//                 YYYY-MM-DDTHH:MM:SS.UUUUUU-0600
//
//     max_ts: specify the latest timestamp for the range of the query.
//         format: YYYY-MM-DDTHH:MM:SS.UUUUUUZ
//                 YYYY-MM-DDTHH:MM:SS.UUUUUU-0600
func (c *ApiContext) AlertsHandler(w *ResponseWriter, r *http.Request) error {

	options := core.AlertQueryOptions{}

	tags := r.FormValue("tags")
	if tags != "" {
		for _, tag := range strings.Split(tags, ",") {
			if strings.HasPrefix(tag, "-") {
				options.MustNotHaveTags = append(options.MustNotHaveTags,
					strings.TrimPrefix(tag, "-"))
			} else {
				options.MustHaveTags = append(options.MustHaveTags, tag)
			}
		}
	}

	args, err := parseCommonRequestArgs(r)
	if err != nil {
		return err
	}
	if args.TimeRange != "" && !(args.MinTs.IsZero() && args.MaxTs.IsZero()) {
		return fmt.Errorf("time_range not allowed with min_ts or max_ts")
	}
	options.MinTs = args.MinTs
	options.MaxTs = args.MaxTs
	options.TimeRange = args.TimeRange

	options.QueryString = r.FormValue("query_string")
	if options.QueryString == "" {
		options.QueryString = r.FormValue("queryString")
	}

	alerts, err := c.appContext.DataStore.AlertQuery(options)
	if err != nil {
		return err
	}

	response := map[string]interface{}{
		"alerts": alerts,
	}

	return w.OkJSON(response)
}

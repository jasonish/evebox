/* Copyright (c) 2016 Jason Ish
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

package server

import (
	"github.com/jasonish/evebox/core"
	"net/http"
	"strings"
)

func AlertsHandler(appContext AppContext, r *http.Request) interface{} {

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

	options.QueryString = r.FormValue("queryString")
	options.TimeRange = r.FormValue("timeRange")

	//results, err := appContext.AlertQueryService.AlertQuery(options)
	results, err := appContext.DataStore.AlertQuery(options)
	if err != nil {
		return err
	}
	return results
}

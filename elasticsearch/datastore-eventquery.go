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

package elasticsearch

import (
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/log"
)

func (s *DataStore) EventQuery(options core.EventQueryOptions) (interface{}, error) {
	query := NewEventQuery()

	query.MustNot(TermQuery("event_type", "stats"))

	if options.Order == "asc" {
		query.SortBy("@timestamp", "asc")
	} else {
		query.SortBy("@timestamp", "desc")
	}

	if options.Size > 0 {
		query.Size = options.Size
	} else {
		query.Size = 500
	}

	if options.QueryString != "" {
		query.AddFilter(QueryString(options.QueryString))
	}

	if !options.MinTs.IsZero() {
		query.AddFilter(RangeGte("@timestamp",
			FormatTimestampUTC(options.MinTs)))
	}

	if !options.MaxTs.IsZero() {
		query.AddFilter(RangeLte("@timestamp",
			FormatTimestampUTC(options.MaxTs)))
	}

	if options.EventType != "" {
		query.AddFilter(TermQuery("event_type", options.EventType))
	}

	response, err := s.es.Search(query)
	if err != nil {
		log.Error("%v", err)
	}

	hits := response.Hits.Hits

	return map[string]interface{}{
		"data": hits,
	}, nil
}

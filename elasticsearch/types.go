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
	"strings"
	"strconv"
)

// Response to an Elastic Search ping (GET /)
type PingResponse struct {
	Name        string `json:"name"`
	ClusterName string `json:"cluster_name"`
	Version     struct {
		Number string `json:"number"`
	} `json:"version"`
	Tagline string `json:"tagline"`
}

func (p PingResponse) MajorVersion() (int64) {
	version := p.Version.Number
	parts := strings.Split(version, ".")
	major, err := strconv.ParseInt(parts[0], 10, 64)
	if err != nil {
		return -1
	} else {
		return major
	}
}

type BulkCreateHeader struct {
	Create struct {
		Index string `json:"_index"`
		Type  string `json:"_type"`
		Id    string `json:"_id"`
	} `json:"create"`
}

// Struct representing a response to a _bulk request.
type BulkResponse struct {
	Took   uint64                   `json:"took"`
	Errors bool                     `json:"errors"`
	Items  []map[string]interface{} `json:"items"`
}

type Hits struct {
	Total uint64                   `json:"total"`
	Hits  []map[string]interface{} `json:"hits"`
}

type SearchResponse struct {
	Shards   map[string]interface{} `json:"_shards"`
	TimedOut bool                   `json:"timed_out"`
	Took     uint64                 `json:"took"`
	Hits     Hits                   `json:"hits"`
	ScrollId string                 `json:"_scroll_id"`
}

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

const (
	SORT_ASC  = "asc"
	SORT_DESC = "desc"
)

// Type alias for JSON building.
type mapping map[string]interface{}

// Type alias for JSON building.
type list []interface{}

// Response to an Elastic Search ping (GET /)
type PingResponse struct {
	Name        string `json:"name"`
	ClusterName string `json:"cluster_name"`
	Version     struct {
		Number string `json:"number"`
	} `json:"version"`
	Tagline string `json:"tagline"`
}

// Response object for generic responses.
type ResponseObject struct {
	val interface{}
}

func (o ResponseObject) Get(key string) ResponseObject {
	val := o.val.(map[string]interface{})[key]
	return ResponseObject{val: val}
}

func (o ResponseObject) Value() interface{} {
	return o.val
}

// A generic response.
type Response struct {
	body map[string]interface{}
}

func (r *Response) Get(key string) ResponseObject {
	return ResponseObject{val: r.body[key]}
}

func NewResponse(body map[string]interface{}) *Response {
	return &Response{
		body: body,
	}
}

type SearchResponse struct {
	Took     uint64 `json:"took"`
	TimedOut bool   `json:"timed_out"`
	Shards   struct {
		Failed     uint64 `json:"failed"`
		Successful uint64 `json:"successful"`
		Total      uint64 `json:"total"`
	} `json:"_shards"`
	Hits struct {
		Hits []map[string]interface{} `json:"hits"`
	} `json:"hits"`

	// The raw response.
	raw string
}

func (sr SearchResponse) Raw() string {
	return sr.raw
}

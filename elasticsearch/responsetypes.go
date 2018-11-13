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
	"bytes"
	"encoding/json"
	"github.com/jasonish/evebox/util"
	"io/ioutil"
	"net/http"
	"strconv"
	"strings"
)

type Hits struct {
	Total uint64                   `json:"total"`
	Hits  []map[string]interface{} `json:"hits"`
}

type Response struct {
	// Ping response fields.
	Name        string `json:"name,omitempty"`
	ClusterName string `json:"cluster_name,omitempty"`
	ClusterUuid string `json:"cluster_uuid,omitempty"`
	Version     struct {
		Number string `json:"number,omitempty"`
	} `json:"version"`
	Tagline string `json:"tagline,omitempty"`

	// Bulk response fields.
	Errors bool                     `json:"errors,omitempty"`
	Items  []map[string]interface{} `json:"items,omitempty"`

	Shards       map[string]interface{} `json:"_shards,omitempty"`
	ScrollId     string                 `json:"_scroll_id,omitempty"`
	TimedOut     bool                   `json:"timed_out,omitempty"`
	Took         uint64                 `json:"took,omitempty"`
	Hits         Hits                   `json:"hits,omitempty"`
	Aggregations util.JsonMap           `json:"aggregations,omitempty"`

	// A search may result in an error.
	Error map[string]interface{} `json:"error,omitempty"`

	Status int `json:"status,omitempty"`

	Raw []byte `json:"-"`
}

func (r *Response) GetFirstRootCause() string {
	reason := util.JsonMap(r.Error).GetMapSlice("root_cause").First().GetString("reason")
	return reason
}

func DecodeResponse(r *http.Response) (*Response, error) {
	raw, err := ioutil.ReadAll(r.Body)
	if err != nil {
		return nil, err
	}

	response := &Response{}

	if strings.HasPrefix(r.Header.Get("content-type"), "application/json") {
		decoder := json.NewDecoder(bytes.NewReader(raw))
		decoder.UseNumber()
		if err := decoder.Decode(response); err != nil {
			return nil, err
		}
	}

	response.Raw = raw

	return response, nil
}

func DecodeResponseAsError(r *http.Response) error {
	raw, err := ioutil.ReadAll(r.Body)
	if err != nil {
		return err
	}

	response := &Response{}

	if strings.HasPrefix(r.Header.Get("content-type"), "application/json") {
		decoder := json.NewDecoder(bytes.NewReader(raw))
		decoder.UseNumber()

		if err := decoder.Decode(response); err != nil {
			return err
		}
	}

	response.Raw = raw

	return response.AsError()
}

func (r *Response) HasErrors() bool {
	if r.Error != nil {
		return true
	}
	if len(r.Error) > 0 {
		return true
	}
	return false
}

func (r *Response) IsError() bool {
	return r.Error != nil
}

func (r *Response) AsError() *ErrorResponse {
	return &ErrorResponse{r}
}

type ErrorResponse struct {
	*Response
}

func (e *ErrorResponse) Error() string {
	return string(e.Raw)
}

// PingResponse represents the response to an Elastic Search ping (GET /).
type PingResponse struct {
	*Response
}

// MajorVersion returns the major version of Elastic Search as found
// in the PingResponse.
func (p PingResponse) MajorVersion() int64 {
	version := p.Version.Number
	parts := strings.Split(version, ".")
	major, err := strconv.ParseInt(parts[0], 10, 64)
	if err != nil {
		return -1
	}
	return major

}

// ParseVersion parses the Elastic Search version in the ping response
// returning the major and minor versions.
func (p PingResponse) ParseVersion() (int64, int64) {
	majorVersion := int64(0)
	minorVersion := int64(0)
	parts := strings.Split(p.Version.Number, ".")
	if len(parts) > 0 {
		version, err := strconv.ParseInt(parts[0], 10, 64)
		if err == nil {
			majorVersion = version
		}
	}
	if len(parts) > 1 {
		version, err := strconv.ParseInt(parts[1], 10, 64)
		if err == nil {
			minorVersion = version
		}
	}
	return majorVersion, minorVersion
}

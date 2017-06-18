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
	"github.com/pkg/errors"
	"io"
	"io/ioutil"
	"net/http"
	"strconv"
	"strings"
)

// PingResponse represents the response to an Elastic Search ping (GET /).
type PingResponse struct {
	Name        string `json:"name"`
	ClusterName string `json:"cluster_name"`
	Version     struct {
		Number string `json:"number"`
	} `json:"version"`
	Tagline string `json:"tagline"`
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

// BulkCreateHeader represents the JSON used to prefix a document to be indexed
// in the bulk request.
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

type ElasticSearchError struct {
	// The raw error body as returned from the server.
	Raw string
}

func (e ElasticSearchError) Error() string {
	return e.Raw
}

func NewElasticSearchError(response *http.Response) ElasticSearchError {

	error := ElasticSearchError{}

	raw, _ := ioutil.ReadAll(response.Body)
	if raw != nil {
		error.Raw = string(raw)
	}

	return error
}

type RawResponse struct {
	json util.JsonMap
	raw  []byte
}

func DecodeRawResponse(r io.Reader) (*RawResponse, error) {
	raw, err := ioutil.ReadAll(r)
	if err != nil {
		return nil, errors.Wrap(err, "failed to read response")
	}
	response := &RawResponse{raw: raw}
	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()
	err = decoder.Decode(&response.json)
	if err != nil {
		return response, errors.Wrap(err, "failed to decode response")
	}
	return response, nil
}

func (r RawResponse) RawString() string {
	return string(r.raw)
}

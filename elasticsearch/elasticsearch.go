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
	"encoding/json"
	"io/ioutil"
	"net/http"
)

type ElasticSearch struct {
	baseUrl    string
	httpClient *http.Client
	index      string
}

func New(url string) *ElasticSearch {
	return &ElasticSearch{
		baseUrl:    url,
		httpClient: &http.Client{},
		index:      "logstash",
	}
}

func (es *ElasticSearch) Ping() (*PingResponse, error) {

	req, err := http.NewRequest("GET", es.baseUrl, nil)
	if err != nil {
		return nil, err
	}
	response, err := es.httpClient.Do(req)
	if err != nil {
		return nil, err
	}

	if response.StatusCode != 200 {
		return nil, NewElasticSearchError(response)
	}

	decoder := json.NewDecoder(response.Body)
	decoder.UseNumber()
	var body PingResponse
	if err := decoder.Decode(&body); err != nil {
		return nil, err
	}

	return &body, nil
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

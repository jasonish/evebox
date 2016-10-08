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
	"net/http"
	"fmt"
	"github.com/GeertJohan/go.rice"
	"io"
	"strings"
	"bytes"
	"io/ioutil"
	"net"
	"crypto/tls"
)

type ElasticSearch struct {
	baseUrl          string
	httpClient       *http.Client
	DisableCertCheck bool
}

func New(url string) *ElasticSearch {
	es := &ElasticSearch{
		baseUrl:    url,
		httpClient: &http.Client{},
	}
	es.httpClient.Transport = &http.Transport{DialTLS:es.DialTLS}
	return es
}

func (es *ElasticSearch) DialTLS(network string, addr string) (net.Conn, error) {
	return tls.Dial(network, addr, &tls.Config{
		InsecureSkipVerify: es.DisableCertCheck,
	})
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

func (es *ElasticSearch) PUT(path string, body interface{}) (error) {

	var bodyAsReader io.Reader

	bodyAsReader = nil

	if body == nil {
		bodyAsReader = nil
	} else {
		switch body := body.(type) {
		case string:
			bodyAsReader = strings.NewReader(body)
		case []byte:
			bodyAsReader = bytes.NewReader(body)
		default:
			buf := new(bytes.Buffer)
			encoder := json.NewEncoder(buf)
			encoder.Encode(body)
			bodyAsReader = buf
		}
	}

	request, err := http.NewRequest("PUT",
		fmt.Sprintf("%s/%s", es.baseUrl, path), bodyAsReader)
	if err != nil {
		return err
	}

	response, err := es.httpClient.Do(request)
	if err != nil {
		return err
	}
	if response.StatusCode != http.StatusOK {
		return NewElasticSearchError(response)
	}

	io.Copy(ioutil.Discard, response.Body)

	return nil
}

func (es *ElasticSearch) CheckTemplate(name string) (exists bool, err error) {
	request, err := http.NewRequest("HEAD",
		fmt.Sprintf("%s/_template/%s", es.baseUrl, name), nil)
	if err != nil {
		return exists, err
	}
	response, err := es.httpClient.Do(request)
	if err != nil {
		return exists, err
	}
	exists = response.StatusCode == 200
	return exists, nil
}

func (es *ElasticSearch) LoadTemplate(index string) error {

	templateBox, err := rice.FindBox("static")
	if err != nil {
		return err
	}

	templateFile, err := templateBox.Open("es2x-template.json")
	if err != nil {
		return err
	}

	decoder := json.NewDecoder(templateFile)
	decoder.UseNumber()

	var template map[string]interface{}
	err = decoder.Decode(&template)
	if err != nil {
		return err
	}
	template["template"] = fmt.Sprintf("%s-*", index)

	err = es.PUT(fmt.Sprintf("_template/%s", index), template)
	if err != nil {
		return err
	}

	// Success.
	return nil
}
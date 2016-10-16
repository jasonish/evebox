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
	"crypto/tls"
	"encoding/json"
	"fmt"
	"io"
	"io/ioutil"
	"net"
	"net/http"
	"strings"

	"github.com/GeertJohan/go.rice"
)

type ElasticSearch struct {
	baseUrl          string
	httpClient       *http.Client
	DisableCertCheck bool
	username         string
	password         string
	EventIndex       string
}

func New(url string) *ElasticSearch {
	es := &ElasticSearch{
		baseUrl:    url,
		httpClient: &http.Client{},
	}
	es.httpClient.Transport = &http.Transport{DialTLS: es.DialTLS}
	return es
}

func (i *ElasticSearch) SetUsernamePassword(username ...string) error {
	if len(username) == 1 {
		parts := strings.SplitN(username[0], ":", 2)
		if len(parts) < 2 {
			return fmt.Errorf("bad format")
		}
		i.username = parts[0]
		i.password = parts[1]
	} else {
		i.username = username[0]
		i.password = username[1]
	}
	return nil
}

func (es *ElasticSearch) DialTLS(network string, addr string) (net.Conn, error) {
	return tls.Dial(network, addr, &tls.Config{
		InsecureSkipVerify: es.DisableCertCheck,
	})
}

func (es *ElasticSearch) httpDo(request *http.Request) (*http.Response, error) {
	if es.username != "" || es.password != "" {
		request.SetBasicAuth(es.username, es.password)
	}
	return es.httpClient.Do(request)
}

func (es *ElasticSearch) Head(url string) (*http.Response, error) {
	request, err := http.NewRequest("HEAD", url, nil)
	if err != nil {
		return nil, err
	}
	return es.httpDo(request)
}

func (es *ElasticSearch) Get(url string) (*http.Response, error) {
	request, err := http.NewRequest("GET", url, nil)
	if err != nil {
		return nil, err
	}
	return es.httpDo(request)
}

func (es *ElasticSearch) DeleteWithStringBody(path string, bodyType string, body string) (*http.Response, error) {
	request, err := http.NewRequest("DELETE",
		fmt.Sprintf("%s/%s", es.baseUrl, path),
		strings.NewReader(body))
	if err != nil {
		return nil, err
	}
	request.Header.Set("Content-Type", bodyType)
	return es.httpDo(request)
}

func (es *ElasticSearch) Post(url string, bodyType string, body io.Reader) (*http.Response, error) {
	request, err := http.NewRequest("POST", url, body)
	if err != nil {
		return nil, err
	}
	request.Header.Set("Content-Type", bodyType)
	return es.httpDo(request)
}

func (es *ElasticSearch) PostString(path string, contentType string, body string) (*http.Response, error) {
	url := fmt.Sprintf("%s/%s", es.baseUrl, path)
	request, err := http.NewRequest("POST", url, strings.NewReader(body))
	if err != nil {
		return nil, err
	}
	request.Header.Set("Content-Type", contentType)
	return es.httpDo(request)
}

func (es *ElasticSearch) PostJson(path string, body interface{}) (*http.Response, error) {
	buf, err := json.Marshal(body)
	if err != nil {
		return nil, err
	}
	url := fmt.Sprintf("%s/%s", es.baseUrl, path)
	return es.Post(url, "application/json", bytes.NewReader(buf))
}

func (es *ElasticSearch) Put(path string, body interface{}) error {

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

	response, err := es.httpDo(request)
	if err != nil {
		return err
	}
	if response.StatusCode != http.StatusOK {
		return NewElasticSearchError(response)
	}

	io.Copy(ioutil.Discard, response.Body)

	return nil
}

func (es *ElasticSearch) Ping() (*PingResponse, error) {

	response, err := es.Get(es.baseUrl)
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

func (es *ElasticSearch) CheckTemplate(name string) (exists bool, err error) {
	response, err := es.Head(fmt.Sprintf("%s/_template/%s", es.baseUrl, name))
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

	err = es.Put(fmt.Sprintf("_template/%s", index), template)
	if err != nil {
		return err
	}

	// Success.
	return nil
}

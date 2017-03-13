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
 * STRICT LIABILITY, TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING
 * IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
 * POSSIBILITY OF SUCH DAMAGE.
 */

package httputil

import (
	"bytes"
	"crypto/tls"
	"encoding/json"
	"fmt"
	"github.com/jasonish/evebox/log"
	"github.com/pkg/errors"
	"io"
	"io/ioutil"
	"net"
	"net/http"
	"strings"
)

type HttpClient struct {
	baseUrl          string
	redirectBaseUrl  string
	username         string
	password         string
	disableCertCheck bool
	httpClient       *http.Client
}

func NewHttpClient() *HttpClient {
	httpClient := &HttpClient{
		httpClient: &http.Client{},
	}
	httpClient.httpClient.CheckRedirect = httpClient.CheckRedirect
	return httpClient
}

func (c *HttpClient) SetBaseUrl(baseUrl string) {
	c.baseUrl = baseUrl
}

func (c *HttpClient) DisableCertCheck(disableCertCheck bool) {
	c.disableCertCheck = disableCertCheck
}

func (c *HttpClient) SetUsernamePassword(username ...string) error {
	if len(username) == 1 {
		parts := strings.SplitN(username[0], ":", 2)
		if len(parts) < 2 {
			return fmt.Errorf("bad format")
		}
		c.username = parts[0]
		c.password = parts[1]
	} else {
		c.username = username[0]
		c.password = username[1]
	}
	return nil
}

func (c *HttpClient) DialTLS(network string, addr string) (net.Conn, error) {
	return tls.Dial(network, addr, &tls.Config{
		InsecureSkipVerify: c.disableCertCheck,
	})
}

func (c *HttpClient) CheckRedirect(request *http.Request, via []*http.Request) error {
	if len(via) >= 10 {
		return errors.New("stopped after 10 redirects")
	}
	location, err := request.Response.Location()
	if err == nil {
		log.Info("Updating redirect base URL to %s", location.String())
		c.redirectBaseUrl = location.String()
	}
	return nil
}

func (c *HttpClient) Do(request *http.Request) (*http.Response, error) {
	if c.username != "" || c.password != "" {
		request.SetBasicAuth(c.username, c.password)
	}
	response, err := c.httpClient.Do(request)
	if err != nil {
		return response, errors.Wrap(err, "")
	}
	return response, err
}

func (c *HttpClient) Request(method string, path string, contentType string, body io.Reader) (*http.Response, error) {
	baseUrl := c.baseUrl
	if c.redirectBaseUrl != "" && (method == "POST" || method == "PUT") {
		baseUrl = c.redirectBaseUrl
	}
	request, err := http.NewRequest(method, fmt.Sprintf("%s/%s", baseUrl, path), body)
	if err != nil {
		return nil, err
	}
	if contentType != "" {
		request.Header.Set("Content-Type", contentType)
	}
	return c.Do(request)
}

func (c *HttpClient) Head(path string) (*http.Response, error) {
	return c.Request("HEAD", path, "", nil)
}

func (c *HttpClient) Get(path string) (*http.Response, error) {
	return c.Request("GET", path, "", nil)
}

func (c *HttpClient) Post(path string, contentType string, body io.Reader) (*http.Response, error) {
	return c.Request("POST", path, contentType, body)
}

func (c *HttpClient) PostJson(path string, body interface{}) (*http.Response, error) {
	buf, err := json.Marshal(body)
	if err != nil {
		return nil, err
	}
	return c.Post(path, "application/json", bytes.NewReader(buf))
}

func (c *HttpClient) PostJsonDecodeResponse(path string, body interface{}, response interface{}) error {
	buf, err := json.Marshal(body)
	if err != nil {
		return err
	}
	r, err := c.Post(path, "application/json", bytes.NewReader(buf))
	if err != nil {
		return err
	}
	decoder := json.NewDecoder(r.Body)
	decoder.UseNumber()
	return decoder.Decode(response)
}

func (c *HttpClient) PostString(path string, contentType string, body string) (*http.Response, error) {
	return c.Post(path, contentType, strings.NewReader(body))
}

func (c *HttpClient) PostBytes(path string, contentType string, body []byte) (*http.Response, error) {
	return c.Post(path, contentType, bytes.NewReader(body))
}

func (c *HttpClient) Put(path string, contentType string, body io.Reader) (*http.Response, error) {
	return c.Request("PUT", path, contentType, body)
}

func (c *HttpClient) PutJson(path string, body interface{}) (*http.Response, error) {
	encoded, err := json.Marshal(body)
	if err != nil {
		return nil, err
	}
	return c.Put(path, "application/json", bytes.NewReader(encoded))
}

func (c *HttpClient) Delete(path string, contentType string, body io.Reader) (*http.Response, error) {
	return c.Request("DELETE", path, contentType, body)
}

func (c *HttpClient) DiscardResponse(response *http.Response) {
	io.Copy(ioutil.Discard, response.Body)
}

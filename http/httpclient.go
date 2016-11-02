package http

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
)

type HttpClient struct {
	baseUrl          string
	username         string
	password         string
	disableCertCheck bool
	httpClient       *http.Client
}

func NewHttpClient() *HttpClient {
	return &HttpClient{
		httpClient: &http.Client{},
	}
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

func (c *HttpClient) Do(request *http.Request) (*http.Response, error) {
	if c.username != "" || c.password != "" {
		request.SetBasicAuth(c.username, c.password)
	}
	return c.httpClient.Do(request)
}

func (c *HttpClient) Request(method string, path string, contentType string, body io.Reader) (*http.Response, error) {
	request, err := http.NewRequest(method, fmt.Sprintf("%s/%s", c.baseUrl, path), body)
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

func (c *HttpClient) PostString(path string, contentType string, body string) (*http.Response, error) {
	return c.Post(path, contentType, strings.NewReader(body))
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

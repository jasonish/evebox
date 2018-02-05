/* Copyright (c) 2017 Jason Ish
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

package agent

import (
	"encoding/json"
	"github.com/jasonish/evebox/util"
	"github.com/jasonish/evebox/httpclient"
)

type Client struct {
	httpClient *httpclient.HttpClient
}

func NewClient() *Client {
	client := Client{
		httpClient: httpclient.NewHttpClient(),
	}
	return &client
}

func (c *Client) SetBaseUrl(url string) {
	c.httpClient.SetBaseUrl(url)
}

func (c *Client) SetUsernamePassword(username string, password string) {
	c.httpClient.SetUsernamePassword(username, password)
}

func (c *Client) GetVersion() (*util.JsonMap, error) {
	response, err := c.httpClient.Get("api/1/version")
	if err != nil {
		return nil, err
	}
	var version util.JsonMap
	decoder := json.NewDecoder(response.Body)
	err = decoder.Decode(&version)
	return &version, err
}

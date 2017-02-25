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
	"github.com/pkg/errors"
)

// The "channel" for events to be submitted to the Evebox server.
type EventChannel struct {
	client *Client

	// Raw buffer that is sent to server on commit.
	buf []byte
}

func NewEventChannel(client *Client) *EventChannel {
	eventChannel := EventChannel{}
	eventChannel.client = client
	return &eventChannel
}

func (ec *EventChannel) Commit() (*util.JsonMap, error) {
	response, err := ec.client.httpClient.PostBytes("api/1/submit",
		"application/json", ec.buf)
	if err != nil {
		return nil, err
	}

	if response.StatusCode > 200 {
		return nil, errors.Errorf("unexpected status: %s", response.Status)
	}

	ec.buf = ec.buf[:0]

	var jsonMap util.JsonMap
	decoder := json.NewDecoder(response.Body)
	decoder.UseNumber()
	if err := decoder.Decode(&jsonMap); err != nil {
		return nil, err
	}
	return &jsonMap, nil
}

func (ec *EventChannel) Submit(event interface{}) error {
	rawEvent, err := json.Marshal(event)
	if err != nil {
		return err
	}

	ec.buf = append(ec.buf, rawEvent...)
	ec.buf = append(ec.buf, []byte("\n")...)

	return nil
}

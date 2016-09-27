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
	"fmt"
	"github.com/jasonish/evebox/eve"
	"io/ioutil"
	"math/rand"
	"net/http"
	"time"
)

var chars = []rune("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ")

const AtTimestampFormat = "2006-01-02T15:04:05.999Z"

func init() {
	rand.Seed(time.Now().UnixNano())
}

func GenerateId() string {
	id := make([]rune, 20)
	for i := range id {
		id[i] = chars[rand.Intn(len(chars))]
	}
	return string(id)
}

func AddAtTimestamp(event eve.RawEveEvent) {
	timestamp, err := event.GetTimestamp()
	if err != nil {
		return
	}
	event["@timestamp"] = timestamp.UTC().Format(AtTimestampFormat)
}

func (es *ElasticSearch) IndexRawEveEvent(event eve.RawEveEvent) error {
	id := GenerateId()

	timestamp, err := event.GetTimestamp()
	if err != nil {
		return err
	}
	index := fmt.Sprintf("%s-%s", es.index, timestamp.UTC().Format("2006.01.02"))

	AddAtTimestamp(event)

	var buf bytes.Buffer
	encoder := json.NewEncoder(&buf)
	encoder.Encode(event)
	request, err := http.NewRequest("POST",
		fmt.Sprintf("%s/%s/log/%s", es.baseUrl, index, id), &buf)
	if err != nil {
		return err
	}
	response, err := es.httpClient.Do(request)
	if err != nil {
		return err
	}

	// Required for connection re-use.
	ioutil.ReadAll(response.Body)
	response.Body.Close()

	return nil
}

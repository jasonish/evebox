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

package eve

import (
	"bytes"
	"encoding/base64"
	"encoding/json"
	"fmt"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/util"
	"strings"
	"time"
)

// A EveEvent is an Eve event decoded into map[string]interface{} which
// contains all the data in its raw format.
type EveEvent map[string]interface{}

func NewEveEventFromBytes(b []byte) (event EveEvent, err error) {

	decoder := json.NewDecoder(bytes.NewReader(b))
	decoder.UseNumber()
	if err := decoder.Decode(&event); err != nil {
		return nil, err
	}

	// Create empty tags if it doesn't exist.
	if event["tags"] == nil {
		event["tags"] = []interface{}{}
	}

	// Attempt to parse the timestamp, fail the decode if it can't be
	// parsed.
	timestamp, err := event.parseTimestamp()
	if err != nil {
		return nil, err
	}

	// Cache the timestamp.
	event["__parsed_timestamp"] = timestamp

	return event, nil
}

func NewEveEventFromString(s string) (event EveEvent, err error) {
	return NewEveEventFromBytes([]byte(s))
}

func (e EveEvent) MarshalJSON() ([]byte, error) {
	event := map[string]interface{}{}
	for key, val := range e {
		if strings.HasPrefix(key, "__") {
			continue
		}
		event[key] = val
	}
	return json.Marshal(event)
}

func (e EveEvent) parseTimestamp() (time.Time, error) {
	tsstring, ok := e["timestamp"].(string)
	if !ok {
		return time.Time{}, fmt.Errorf("not a string")
	}
	return ParseTimestamp(tsstring)
}

func (e EveEvent) Timestamp() time.Time {
	return e["__parsed_timestamp"].(time.Time)
}

func (e EveEvent) SetTimestamp(ts time.Time) {
	e["timestamp"] = FormatTimestamp(ts)
	e["__parsed_timestamp"] = ts
}

func (e EveEvent) EventType() string {
	if eventType, ok := e["event_type"].(string); ok {
		return eventType
	}
	return ""
}

func (e EveEvent) Packet() []byte {
	packet, ok := e["packet"].(string)
	if !ok {
		return nil
	}
	buf, err := base64.StdEncoding.DecodeString(packet)
	if err != nil {
		return nil
	}
	return buf
}

func (e EveEvent) Proto() string {
	return e.GetString("proto")
}

func (e EveEvent) SrcIp() string {
	return e.GetString("src_ip")
}

func (e EveEvent) DestIp() string {
	return e.GetString("dest_ip")
}

func (e EveEvent) SrcPort() uint16 {
	return asUint16(e["src_port"])
}

func (e EveEvent) DestPort() uint16 {
	return asUint16(e["dest_port"])
	return e["dest_port"].(uint16)
}

func (e EveEvent) IcmpType() uint8 {
	return uint8(asUint16(e["icmp_type"]))
}

func (e EveEvent) IcmpCode() uint8 {
	return uint8(asUint16(e["icmp_code"]))
}

func (e EveEvent) Payload() []byte {
	packet, ok := e["payload"].(string)
	if !ok {
		return nil
	}
	buf, err := base64.StdEncoding.DecodeString(packet)
	if err != nil {
		return nil
	}
	return buf
}

func (e EveEvent) GetMap(key string) util.JsonMap {
	return util.JsonMap(e).GetMap(key)
}

func (e EveEvent) GetString(key string) string {
	return util.JsonMap(e).GetString(key)
}

func (e EveEvent) GetAlert() util.JsonMap {
	return util.JsonMap(e).GetMap("alert")
}

func (e EveEvent) GetAlertSignatureId() (uint64, bool) {
	ruleId, ok := e.GetMap("alert").Get("signature_id").(json.Number)
	if ok {
		asInt64, err := ruleId.Int64()
		if err == nil {
			return uint64(asInt64), true
		}
	}
	return 0, false
}

func (e EveEvent) AddTag(tag string) {
	if e["tags"] == nil {
		log.Println("Tags is null...")
		e["tags"] = []interface{}{}
	}
	tags := e["tags"].([]interface{})
	tags, ok := e["tags"].([]interface{})
	if !ok {
		log.Warning("Failed to convert tags to []interface{}: %v", e["tags"])
		return
	}
	for _, existing := range tags {
		if existing == tag {
			return
		}
	}
	tags = append(tags, tag)
	e["tags"] = tags
}

func asUint16(in interface{}) uint16 {
	if number, ok := in.(json.Number); ok {
		asInt64, err := number.Int64()
		if err == nil {
			return uint16(asInt64)
		}
	}
	return 0
}

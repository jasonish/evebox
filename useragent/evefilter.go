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

package useragent

import (
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"github.com/ua-parser/uap-go/uaparser"
)

var parser *uaparser.Parser

func init() {
	parser = uaparser.NewFromSaved()
}

type EveUserAgentFilter struct {
	cache map[string]map[string]string
}

func (f *EveUserAgentFilter) setValue(ua map[string]string, name string, value string) {
	switch value {
	case "":
		return
	case "Other":
		return
	default:
		ua[name] = value
	}
}

func (f *EveUserAgentFilter) Filter(event eve.RawEveEvent) {
	if event.EventType() != "http" {
		return
	}

	httpUserAgent := event.GetMap("http").GetString("http_user_agent")
	if httpUserAgent == "" {
		return
	}

	var ua map[string]string

	ua = f.cache[httpUserAgent]
	if ua != nil {
		log.Println("using cached user agent")
		if len(ua) > 0 {
			event.GetMap("http")["user_agent"] = ua
		}
	}

	parsed := parser.Parse(event.GetMap("http").GetString("http_user_agent"))

	ua = map[string]string{}

	f.setValue(ua, "name", parsed.UserAgent.Family)
	f.setValue(ua, "major", parsed.UserAgent.Major)
	f.setValue(ua, "minor", parsed.UserAgent.Minor)
	f.setValue(ua, "patch", parsed.UserAgent.Patch)

	f.setValue(ua, "os", parsed.Os.ToString())
	f.setValue(ua, "os_name", parsed.Os.Family)
	f.setValue(ua, "os_major", parsed.Os.Major)
	f.setValue(ua, "os_minor", parsed.Os.Minor)

	f.setValue(ua, "device", parsed.Device.ToString())

	if len(ua) > 0 {
		event.GetMap("http")["user_agent"] = ua
	}
	if f.cache == nil {
		f.cache = map[string]map[string]string{}
	}
	f.cache[httpUserAgent] = ua
}

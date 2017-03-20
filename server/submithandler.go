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

package server

import (
	"bufio"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/useragent"
	"io"
	"net/http"
)

type SubmitResponse struct {
	Count int
}

// Consumes events from agents and adds them to the database.
func SubmitHandler(appContext AppContext, r *http.Request) interface{} {

	count := 0

	eventSink := appContext.DataStore.GetEveEventSink()
	geoFilter := eve.NewGeoipFilter(appContext.GeoIpService)
	tagsFilter := eve.TagsFilter{}
	uaFilter := useragent.EveUserAgentFilter{}

	reader := bufio.NewReader(r.Body)
	for {
		eof := false
		line, err := reader.ReadBytes('\n')
		if err != nil {
			if err == io.EOF {
				eof = true
			} else {
				log.Error("read error: %v", err)
				return err
			}
		}

		if eof && len(line) == 0 {
			break
		}

		event, err := eve.NewEveEventFromBytes(line)
		if err != nil {
			log.Error("failed to decode event: %v", err)
			return err
		}

		tagsFilter.Filter(event)
		geoFilter.Filter(event)
		uaFilter.Filter(event)

		eventSink.Submit(event)

		count++

		if eof {
			break
		}
	}

	_, err := eventSink.Commit()
	if err != nil {
		log.Error("Failed to commit events: %v", err)
		return err
	}

	log.Debug("Committed %d events from %v", count, r.RemoteAddr)

	return SubmitResponse{
		Count: count,
	}
}

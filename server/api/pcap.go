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

package api

import (
	"fmt"
	"github.com/jasonish/evebox/eve"
	"net/http"
)

func (c *ApiContext) Eve2PcapHandler(w *ResponseWriter, r *http.Request) error {
	var event eve.EveEvent
	var err error
	var pcap []byte

	r.ParseForm()

	if r.Form["event"] == nil {
		return fmt.Errorf("Event parameter not provided.")
	}

	jsonEvent := r.Form["event"][0]
	if jsonEvent != "" {
		event, err = eve.NewEveEventFromBytes([]byte(jsonEvent))
		if err != nil {
			return fmt.Errorf("Failed to decode JSON: %v", err)
		}
	} else {
		return fmt.Errorf("Form field \"event\" not provided.")
	}

	what := r.Form["what"][0] // r.URL.Query().Get("what")
	if what == "" {
		if len(event.Payload()) > 0 {
			what = "payload"
		} else if len(event.Packet()) > 0 {
			what = "packet"
		}
	}

	if what == "payload" {
		if len(event.Payload()) == 0 {
			return fmt.Errorf("Payload conversion requested but event does not contain the payload.")
		}

		pcap, err = eve.EvePayloadToPcap(event)
		if err != nil {
			return fmt.Errorf("Failed to convert payload to pcap: %v", err)
		}

	} else if what == "packet" {
		if len(event.Packet()) == 0 {
			return fmt.Errorf("Packet conversion requested but event not contain the packet.")
		}
		pcap, err = eve.EvePacket2Pcap(event)
		if err != nil {
			return fmt.Errorf("Failed to convert packet to pcap: %v", err)
		}
	}

	w.Header().Set("content-type", "application/vnc.tcpdump.pcap")
	w.Header().Set("content-disposition", "attachment; filename=event.pcap")
	_, err = w.Write(pcap)
	return err
}

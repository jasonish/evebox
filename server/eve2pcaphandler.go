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

package server

import (
	"fmt"
	"github.com/jasonish/evebox"
	"log"
	"net/http"
)

func HttpErrorAndLog(w http.ResponseWriter, r *http.Request, code int,
	format string, v ...interface{}) {
	error := fmt.Sprintf(format, v...)
	log.Printf("%s", error)
	http.Error(w, error, code)
}

func Eve2PcapHandler(w http.ResponseWriter, r *http.Request) {
	var event *evebox.EveEvent
	var err error
	var pcap []byte

	r.ParseForm()

	jsonEvent := r.Form["event"][0]
	if jsonEvent != "" {
		event, err = evebox.NewEveEventFromJson(jsonEvent)
		if err != nil {
			HttpErrorAndLog(w, r, http.StatusBadRequest,
				"Failed to decode JSON:", err)
			return
		}
	} else {
		HttpErrorAndLog(w, r, http.StatusBadRequest,
			"Form field \"event\" not provided.")
		return
	}

	what := r.Form["what"][0] // r.URL.Query().Get("what")
	if what == "" {
		if len(event.Payload) > 0 {
			what = "payload"
		} else if len(event.Packet) > 0 {
			what = "packet"
		}
	}

	if what == "payload" {
		if len(event.Payload) == 0 {
			HttpErrorAndLog(w, r, http.StatusBadRequest,
				"Payload conversion requested but JSON contains no payload.")
			return
		}
		pcap, err = evebox.EvePayloadToPcap(event)
		if err != nil {
			HttpErrorAndLog(w, r, http.StatusInternalServerError,
				"Failed to convert to PCAP: %s", err)
			return
		}
	} else if what == "packet" {
		if len(event.Packet) == 0 {
			HttpErrorAndLog(w, r, http.StatusBadRequest,
				"Packet conversion requested but JSON contains no packet.")
			return
		}
		pcap, err = evebox.EvePacketToPcap(event)
		if err != nil {
			HttpErrorAndLog(w, r, http.StatusInternalServerError,
				"Failed to convert to PCAP: %s", err)
			return
		}
	}

	w.Header().Add("content-type", "application/vnd.tcpdump.pcap")
	w.Header().Add("content-disposition",
		"attachment; filename=event.pcap")
	w.Write(pcap)
}

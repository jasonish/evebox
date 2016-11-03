/* Copyright (c) 2014-2015 Jason Ish
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
	"encoding/json"
	"net/http"

	"github.com/jasonish/evebox/log"
)

type ArchiveHandlerRequest struct {
	SignatureId  uint64 `json:"signature_id"`
	SrcIp        string `json:"src_ip"`
	DestIp       string `json:"dest_ip"`
	MinTimestamp string `json:"min_timestamp"`
	MaxTimestamp string `json:"max_timestamp"`
}

func ArchiveHandler(appContext AppContext, r *http.Request) interface{} {
	var request ArchiveHandlerRequest
	decoder := json.NewDecoder(r.Body)
	decoder.UseNumber()
	decoder.Decode(&request)
	err := appContext.ArchiveService.ArchiveAlerts(request.SignatureId, request.SrcIp,
		request.DestIp, request.MinTimestamp, request.MaxTimestamp)
	if err != nil {
		log.Error("%v", err)
		return err
	}
	return HttpOkResponse()
}

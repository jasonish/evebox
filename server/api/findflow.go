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

package api

import (
	"net/http"
)

// Find the flow matching the provided paramters, useful for finding
// the flow for an event.
func (c *ApiContext) FindFlowHandler(w *ResponseWriter, r *http.Request) error {

	request := struct {
		FlowId    uint64 `json:"flowId"`
		Proto     string `json:"proto"`
		Timestamp string `json:"timestamp"`
		SrcIp     string `json:"srcIp"`
		DestIp    string `json:"destIp"`
	}{}

	if err := DecodeRequestBody(r, &request); err != nil {
		return err
	}

	result, err := c.appContext.DataStore.FindFlow(request.FlowId,
		request.Proto, request.Timestamp, request.SrcIp, request.DestIp)
	if err != nil {
		return err
	}

	response := map[string]interface{}{
		"flows": result,
	}
	return w.OkJSON(response)
}

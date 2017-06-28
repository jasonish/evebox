/* Copyright (c) 2016-2017 Jason Ish
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
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/server/sessions"
	"github.com/pkg/errors"
	"net/http"
)

type AlertGroupQueryParameters struct {
	SignatureId  uint64 `json:"signature_id"`
	SrcIp        string `json:"src_ip"`
	DestIp       string `json:"dest_ip"`
	MinTimestamp string `json:"min_timestamp"`
	MaxTimestamp string `json:"max_timestamp"`
}

func (a *AlertGroupQueryParameters) ToCoreAlertGroupQueryParams() (core.AlertGroupQueryParams, error) {

	params := core.AlertGroupQueryParams{}

	if a.MinTimestamp != "" {
		minTimestamp, err := eve.ParseTimestamp(a.MinTimestamp)
		if err != nil {
			return params, errors.Wrap(err, "bad min_timestamp format")
		}
		params.MinTimestamp = minTimestamp
	}

	if a.MaxTimestamp != "" {
		maxTimestamp, err := eve.ParseTimestamp(a.MaxTimestamp)
		if err != nil {
			return params, errors.Wrap(err, "bad max_timestamp format")
		}
		params.MaxTimestamp = maxTimestamp
	}

	params.SignatureID = a.SignatureId
	params.SrcIP = a.SrcIp
	params.DstIP = a.DestIp

	return params, nil
}

// /api/1/alert-group/archive
func (c *ApiContext) AlertGroupArchiveHandler(w *ResponseWriter, r *http.Request) error {
	session := r.Context().Value("session").(*sessions.Session)
	var request AlertGroupQueryParameters

	if err := DecodeRequestBody(r, &request); err != nil {
		return err
	}

	params, err := request.ToCoreAlertGroupQueryParams()
	if err != nil {
		return errors.WithStack(err)
	}

	err = c.appContext.DataStore.ArchiveAlertGroup(params, session.User)
	if err != nil {
		log.Error("%v", err)
		return err
	}
	return w.Ok()
}

func (c *ApiContext) EscalateAlertGroupHandler(w *ResponseWriter, r *http.Request) error {
	session := r.Context().Value("session").(*sessions.Session)

	var request AlertGroupQueryParameters
	if err := DecodeRequestBody(r, &request); err != nil {
		return err
	}

	params, err := request.ToCoreAlertGroupQueryParams()
	if err != nil {
		return errors.WithStack(err)
	}

	if err := c.appContext.DataStore.EscalateAlertGroup(params, session.User); err != nil {
		log.Error("%v", err)
		return errors.WithStack(err)
	}
	return w.Ok()
}

func (c *ApiContext) DeEscalateAlertGroupHandler(w *ResponseWriter, r *http.Request) error {
	session := r.Context().Value("session").(*sessions.Session)
	var request AlertGroupQueryParameters

	if err := DecodeRequestBody(r, &request); err != nil {
		return err
	}

	params, err := request.ToCoreAlertGroupQueryParams()
	if err != nil {
		return errors.WithStack(err)
	}

	if err := c.appContext.DataStore.DeEscalateAlertGroup(params, session.User); err != nil {
		log.Error("%v", err)
		return errors.WithStack(err)
	}
	return w.Ok()
}

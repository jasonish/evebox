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
	"github.com/gorilla/mux"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/server/sessions"
	"github.com/pkg/errors"
	"net/http"
)

type CommentOnAlertGroupRequest struct {
	AlertGroup AlertGroupQueryParameters `json:"alert_group"`
	Comment    string                    `json:"comment"`
}

func (c *ApiContext) CommentOnAlertGroupHandler(w *ResponseWriter, r *http.Request) error {
	session := r.Context().Value("session").(*sessions.Session)

	var request CommentOnAlertGroupRequest
	if err := DecodeRequestBody(r, &request); err != nil {
		return err
	}

	params, err := request.AlertGroup.ToCoreAlertGroupQueryParams()
	if err != nil {
		return errors.WithStack(err)
	}

	err = c.appContext.DataStore.CommentOnAlertGroup(params, session.User, request.Comment)
	if err != nil {
		log.Error("%v", err)
		return errors.WithStack(err)
	}

	log.Info("Comment on alert group by user %s", session.Username())

	return w.Ok()
}

type CommentOnEventIdRequest struct {
	Comment string `json:"comment"`
}

func (c *ApiContext) CommentOnEventHandler(w *ResponseWriter, r *http.Request) error {
	session := r.Context().Value("session").(*sessions.Session)
	eventId := mux.Vars(r)["id"]

	var request CommentOnEventIdRequest
	if err := DecodeRequestBody(r, &request); err != nil {
		return err
	}

	log.Info("Got comment on event %s comment from user %s", eventId, session.Username())

	if err := c.appContext.DataStore.CommentOnEventId(eventId, session.User, request.Comment); err != nil {
		log.Error("%v", err)
		return errors.WithStack(err)
	}

	return w.Ok()
}

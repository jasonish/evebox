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

package auth

import (
	"encoding/base64"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/server/sessions"
	"github.com/satori/go.uuid"
	"net/http"
	"strings"
)

const SESSION_KEY = "x-evebox-session-id"

type AuthenticationRequiredResponse struct {
	Types []string `json:"types"`
}

type Authenticator interface {
	Login(w http.ResponseWriter, r *http.Request) *sessions.Session
	Authenticate(w http.ResponseWriter, r *http.Request) *sessions.Session
}

func findSession(sessionStore *sessions.SessionStore, r *http.Request) *sessions.Session {
	sessionId := r.Header.Get(SESSION_KEY)
	if sessionId != "" {
		session, err := sessionStore.Get(sessionId)
		if err == nil && session != nil {
			return session
		}
		log.Info("Did not find session for ID %s", sessionId)
	}
	return nil
}

func generateSessionId() string {
	id := base64.StdEncoding.EncodeToString(uuid.NewV4().Bytes())
	id = strings.Replace(id, "=", "", -1)
	return id
}

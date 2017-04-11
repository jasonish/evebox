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
	"encoding/json"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/server/sessions"
	"github.com/spf13/viper"
	"net/http"
)

type UsernameAuthenticator struct {
	sessionStore *sessions.SessionStore
}

func NewUsernameAuthenticator(sessionStore *sessions.SessionStore) *UsernameAuthenticator {
	return &UsernameAuthenticator{
		sessionStore: sessionStore,
	}
}

func (a *UsernameAuthenticator) WriteStatusUnauthorized(w http.ResponseWriter) {
	w.WriteHeader(http.StatusUnauthorized)
	encoder := json.NewEncoder(w)

	loginMessage := viper.GetString("authentication.login-message")

	response := map[string]interface{}{
		"status": http.StatusUnauthorized,
		"authentication": AuthenticationRequiredResponse{
			Types: []string{
				"username",
			},
		},
	}
	if loginMessage != "" {
		response["login_message"] = loginMessage
	}

	encoder.Encode(response)
}

func (a *UsernameAuthenticator) Login(w http.ResponseWriter, r *http.Request) *sessions.Session {
	username := r.FormValue("username")
	if username == "" {
		log.Warning("Login request with no username.")
		a.WriteStatusUnauthorized(w)
		return nil
	}
	log.Info("User %s logged in.", username)
	session := &sessions.Session{
		Id:       generateSessionId(),
		Username: username,
	}
	a.sessionStore.Put(session)
	return session
}

func (a *UsernameAuthenticator) Authenticate(w http.ResponseWriter, r *http.Request) *sessions.Session {

	session := findSession(a.sessionStore, r)
	if session != nil {
		return session
	}

	username, _, ok := r.BasicAuth()
	if ok && username != "" {
		session := &sessions.Session{
			Id:       generateSessionId(),
			Username: username,
		}
		a.sessionStore.Put(session)
		return session
	}

	a.WriteStatusUnauthorized(w)
	return nil
}

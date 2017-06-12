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
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/server/sessions"
	"github.com/pkg/errors"
	"github.com/spf13/viper"
	"net/http"
)

type UsernamePasswordAuthenticator struct {
	sessionStore *sessions.SessionStore
	userStore    core.UserStore
}

func NewUsernamePasswordAuthenticator(sessionStore *sessions.SessionStore,
	userStore core.UserStore) *UsernamePasswordAuthenticator {
	return &UsernamePasswordAuthenticator{
		sessionStore: sessionStore,
		userStore:    userStore,
	}
}

func (a *UsernamePasswordAuthenticator) WriteStatusUnauthorized(w http.ResponseWriter) {
	w.WriteHeader(http.StatusUnauthorized)
	encoder := json.NewEncoder(w)

	loginMessage := viper.GetString("authentication.login-message")

	response := map[string]interface{}{
		"status": http.StatusUnauthorized,
		"authentication": AuthenticationRequiredResponse{
			Types: []string{
				"usernamepassword",
			},
		},
	}
	if loginMessage != "" {
		response["login_message"] = loginMessage
	}

	encoder.Encode(response)
}

func (a *UsernamePasswordAuthenticator) Login(r *http.Request) (*sessions.Session, error) {
	username := r.FormValue("username")
	if username == "" {
		return nil, ErrNoUsername
	}

	password := r.FormValue("password")
	if password == "" {
		return nil, ErrNoPassword
	}

	user, err := a.userStore.FindByUsernamePassword(username, password)
	if err != nil {
		return nil, errors.Wrap(err, "bad username or password")
	}

	session := a.sessionStore.NewSession()
	session.User = user
	session.RemoteAddr = r.RemoteAddr

	a.sessionStore.Put(session)

	return session, nil
}

func (a *UsernamePasswordAuthenticator) Authenticate(w http.ResponseWriter, r *http.Request) *sessions.Session {
	session := a.sessionStore.FindSession(r)
	if session != nil {
		if session.User.IsValid() {
			return session
		}
		log.Warning("Found session, but user is invalid.")
	}

	username, password, ok := r.BasicAuth()
	if ok && username != "" && password != "" {
		log.Debug("Authenticating user [%s] with basic auth",
			username)
		user, err := a.userStore.FindByUsernamePassword(username,
			password)
		if err != nil {
			log.Error("User %s failed to login: %v", err)
			a.WriteStatusUnauthorized(w)
			return nil
		}
		session := &sessions.Session{
			Id:         a.sessionStore.GenerateID(),
			User:       user,
			RemoteAddr: r.RemoteAddr,
		}
		a.sessionStore.Put(session)
		return session
	}

	a.WriteStatusUnauthorized(w)
	return nil
}

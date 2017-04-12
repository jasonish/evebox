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
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/server/sessions"
	"net/http"
)

const username = "anonymous"

// The anonymous authenticator is used when no authentication is desired. Each
// http request will be authenticated with a session without having to login.
type AnonymousAuthenticator struct {
	sessionStore *sessions.SessionStore
}

func NewAnonymousAuthenticator(sessionStore *sessions.SessionStore) *AnonymousAuthenticator {
	return &AnonymousAuthenticator{
		sessionStore: sessionStore,
	}
}

func (a *AnonymousAuthenticator) login(username string) *sessions.Session {
	session := &sessions.Session{
		Id:       a.sessionStore.GenerateID(),
		Username: username,
		User: core.User{
			Username: username,
			Id:       username,
		},
	}
	a.sessionStore.Put(session)
	return session
}

func (a *AnonymousAuthenticator) Login(r *http.Request) (*sessions.Session, error) {
	log.Info("Logging in anonymous user from %v", r.RemoteAddr)
	return a.login(username), nil
}

func (a *AnonymousAuthenticator) Authenticate(w http.ResponseWriter, r *http.Request) *sessions.Session {

	// Look for an existing session.
	session := a.sessionStore.FindSession(r)
	if session != nil {
		return session
	}

	session, _ = a.Login(r)
	w.Header().Set(SESSION_KEY, session.Id)

	return session
}

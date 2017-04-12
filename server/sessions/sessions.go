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

package sessions

import (
	"encoding/base64"
	"fmt"
	"github.com/jasonish/evebox/core"
	"math/rand"
	"net/http"
	"strings"
	"time"
)

func init() {
	rand.Seed(time.Now().UnixNano())
}

var UnknownSessionIdError = fmt.Errorf("unknown session ID")

type Session struct {
	Id       string
	Username string
	User     core.User
}

type SessionStore struct {
	Header   string
	sessions map[string]*Session
}

func NewSessionStore() *SessionStore {
	sessionStore := &SessionStore{
		sessions: make(map[string]*Session),
	}
	return sessionStore
}

func (s *SessionStore) Get(id string) (*Session, error) {
	if session, ok := s.sessions[id]; ok {
		return session, nil
	}
	return nil, UnknownSessionIdError
}

func (s *SessionStore) Put(session *Session) {
	s.sessions[session.Id] = session
}

func (s *SessionStore) Delete(session *Session) {
	delete(s.sessions, session.Id)
}

func (s *SessionStore) GenerateID() string {
	bytes := make([]byte, 64)
	for i := 0; i < cap(bytes); i++ {
		bytes[i] = byte(rand.Intn(255))
	}
	sessionId := base64.StdEncoding.EncodeToString(bytes)

	// We're not converting this back, so we really don't need the
	// padding.
	sessionId = strings.Replace(sessionId, "=", "", -1)

	return sessionId
}

func (s *SessionStore) FindSession(r *http.Request) *Session {
	sessionId := r.Header.Get(s.Header)
	if sessionId != "" {
		session, err := s.Get(sessionId)
		if err == nil && session != nil {
			return session
		}
	}
	return nil
}

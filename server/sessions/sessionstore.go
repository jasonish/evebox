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
	"github.com/jasonish/evebox/log"
	"golang.org/x/sync/syncmap"
	"math/rand"
	"net/http"
	"strings"
	"time"
)

// Session timeout in seconds. Default 1 hour.
const TIMEOUT = 3600

func init() {
	rand.Seed(time.Now().UnixNano())
}

type SessionStore struct {
	Header   string
	sessions syncmap.Map
}

func NewSessionStore() *SessionStore {
	sessionStore := &SessionStore{}
	return sessionStore
}

// Reap will remove expired sessions.
func (s *SessionStore) Reap() {
	now := time.Now()

	s.sessions.Range(func(key interface{}, value interface{}) bool {
		session, ok := value.(*Session)
		if ok && now.After(session.Expires) {
			//log.Info("Expiring session %s", session.String())
			log.InfoWithFields(log.Fields{
				"username": session.User.Username,
				"addr":     session.RemoteAddr,
			}, "Expiring session")
			s.Delete(session)
		}
		if !ok {
			log.Warning("Deleting session that didn't assert as session type")
			s.sessions.Delete(key)
		}
		return true
	})
}

func (s *SessionStore) Get(id string) *Session {
	val, ok := s.sessions.Load(id)
	if ok {
		session := val.(*Session)
		s.setSessionTimeout(session)
		return val.(*Session)
	}
	return nil
}

func (s *SessionStore) Put(session *Session) {
	s.sessions.Store(session.Id, session)
}

func (s *SessionStore) Delete(session *Session) {
	s.sessions.Delete(session.Id)
}

func (s *SessionStore) DeleteById(id string) {
	s.sessions.Delete(id)
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

func (s *SessionStore) setSessionTimeout(session *Session) {
	session.Expires = time.Now().Add(TIMEOUT * time.Second)
}

// NewSession creates a new session with a session ID. It DOES NOT add the
// session to the session store.
func (s *SessionStore) NewSession() *Session {
	session := NewSession()
	session.Id = s.GenerateID()
	s.setSessionTimeout(session)
	return session
}

func (s *SessionStore) FindSession(r *http.Request) *Session {
	sessionId := r.Header.Get(s.Header)

	if sessionId == "" {
		cookie, err := r.Cookie(s.Header)
		if err == nil && cookie.Value != "" {
			sessionId = cookie.Value
		}
	}

	if sessionId != "" {
		session := s.Get(sessionId)
		if session != nil {
			return session
		}
	}
	return nil
}

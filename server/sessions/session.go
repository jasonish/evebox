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
	"fmt"
	"github.com/jasonish/evebox/core"
	"sync"
	"time"
)

type Session struct {
	Id         string
	User       core.User
	RemoteAddr string
	expires    time.Time
	Other      map[string]interface{}
	lock       sync.Mutex
}

func NewSession() *Session {
	session := &Session{}
	session.Other = make(map[string]interface{})
	return session
}

func (s *Session) Username() string {
	return s.User.Username
}

func (s *Session) String() string {
	return fmt.Sprintf("{Id: %s; Username: %s}", s.Id, s.User.Username)
}

func (s *Session) UpdateExpires(newExpire time.Time) {
	s.lock.Lock()
	defer s.lock.Unlock()
	s.expires = newExpire
}

func (s *Session) GetExpires() time.Time {
	s.lock.Lock()
	defer s.lock.Unlock()
	return s.expires
}
package sessions

import "fmt"

var UnknownSessionIdError = fmt.Errorf("unknown session ID")

type Session struct {
	Id       string
	Username string
}

type SessionStore struct {
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

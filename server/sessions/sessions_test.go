package sessions

import (
	"github.com/stretchr/testify/require"
	"testing"
	"time"
)

func TestSessionExpire(t *testing.T) {
	r := require.New(t)

	store := NewSessionStore()
	session := store.NewSession()
	store.Put(session)

	r.NotNil(store.Get(session.Id), "session should not be nil")

	// Reap, this is too soon for a timeout...
	store.Reap()
	r.NotNil(store.Get(session.Id), "session should not be nil")

	// Adjust the time so it will timeout.
	session.Expires = time.Now()
	store.Reap()
	r.Nil(store.Get(session.Id), "session should be nil")
}

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

func TestSessionExpireUpdate(t *testing.T) {
	r := require.New(t)
	store := NewSessionStore()

	// Create a session, the expiration will be sometime in the future.
	now := time.Now()
	session := store.NewSession()
	expiration := session.Expires
	r.True(expiration.After(now))

	// Store the session. The expiration should not be updated.
	store.Put(session)
	r.Equal(expiration, session.Expires)

	// Get the session, this should update the expiration time.
	session = store.Get(session.Id)
	r.NotNil(session)
	r.NotEqual(expiration, session.Expires)
}

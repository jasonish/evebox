package configdb

import (
	"github.com/jasonish/evebox/core"
	"github.com/stretchr/testify/assert"
	"testing"
)

func Setup(t *testing.T) *UserStore {
	db, err := NewConfigDB(":memory:")
	if err != nil {
		t.Fatal(err)
	}

	userstore := &UserStore{db.DB}
	return userstore
}

func TestUserNotExist(t *testing.T) {
	userstore := Setup(t)

	_, err := userstore.FindByUsername("no-user")
	assert.Equal(t, err.Error(), "username does not exist")
}

func TestUserAdd(t *testing.T) {
	var err error

	userstore := Setup(t)

	newUser := core.User{}

	_, err = userstore.AddUser(newUser, "")
	assert.NotNil(t, err)

	// Add a valid user.
	username := "test-newUser"
	newUser.Username = username
	_, err = userstore.AddUser(newUser, "password")
	assert.Nil(t, err)

	user, err := userstore.FindByUsername(username)
	assert.Nil(t, err)
	assert.Equal(t, user.Username, username)
}

func TestCheckPassword(t *testing.T) {
	var err error

	userstore := Setup(t)

	username := "username"
	password := "password"

	user := core.User{
		Username: username,
	}
	_, err = userstore.AddUser(user, password)
	assert.Nil(t, err)

	// Check for good password.
	user, err = userstore.FindByUsernamePassword(username, password)
	assert.Nil(t, err)
	assert.Equal(t, user.Username, username)

	// Check for bad password.
	user, err = userstore.FindByUsernamePassword(username, "bad")
	assert.Equal(t, ErrBadPassword, err)
	assert.Equal(t, "", user.Username)
}

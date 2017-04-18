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

package configdb

import (
	"database/sql"
	"fmt"
	"github.com/jasonish/evebox/core"
	"github.com/pkg/errors"
	"github.com/satori/go.uuid"
	"golang.org/x/crypto/bcrypt"
	"strings"
)

var userFields = []string{
	"uuid",
	"username",
	"fullname",
	"email",
	"github_id",
	"github_username",
}

var ErrNoUsername = errors.New("username does not exist")
var ErrNoPassword = errors.New("user has no password")
var ErrBadPassword = errors.New("bad password")

var nilUser = core.User{}

type UserStore struct {
	db *sql.DB
}

func NewUserStore(db *sql.DB) *UserStore {
	return &UserStore{
		db: db,
	}
}

func toNullString(value string) sql.NullString {
	if value != "" {
		return sql.NullString{value, true}
	}
	return sql.NullString{"", false}
}

func toNullInt64(value int64) sql.NullInt64 {
	if value > 0 {
		return sql.NullInt64{value, true}
	}
	return sql.NullInt64{0, false}
}

func encryptPassword(password string) (string, error) {
	hashBytes, err := bcrypt.GenerateFromPassword([]byte(password),
		bcrypt.DefaultCost)
	if err != nil {
		return "", err
	}
	return string(hashBytes), nil
}

func (s *UserStore) AddUser(user core.User, password string) (string, error) {

	noid := ""

	// Validate.
	username := user.Username
	if username == "" {
		return noid, errors.New("username is required")
	}
	fullname := toNullString(user.FullName)
	email := toNullString(user.Email)
	githubId := toNullInt64(user.GitHubID)
	githubUsername := toNullString(user.GitHubUsername)

	var sqlPassword sql.NullString
	if password != "" {
		hash, err := encryptPassword(password)
		if err != nil {
			return noid, errors.Wrap(err,
				"failed to hash password")
		}
		sqlPassword = sql.NullString{hash, true}
	} else {
		sqlPassword = sql.NullString{"", false}
	}

	tx, err := s.db.Begin()
	if err != nil {
		return noid, errors.Wrap(err,
			"failed to create transaction")
	}
	defer tx.Commit()

	id := uuid.NewV4()

	st, err := tx.Prepare(`insert into users (
	      uuid,
	      username,
	      fullname,
	      email,
	      password,
	      github_id,
	      github_username
	    ) values (?, ?, ?, ?, ?, ?, ?)`)
	if err != nil {
		return noid, errors.Wrap(err,
			"failed to prepare user insert statement")
	}
	_, err = st.Exec(id.String(),
		username,
		fullname,
		email,
		sqlPassword,
		githubId,
		githubUsername)
	if err != nil {
		return noid, errors.Wrap(err, "failed to insert user")
	}

	return id.String(), nil
}

// Given a username and password, return a user only if the user exists
// with the given password.
func (s *UserStore) FindByUsernamePassword(username string, password string) (core.User, error) {
	row := s.db.QueryRow(
		"select password from users where username = ?",
		username)
	var hash string
	err := row.Scan(&hash)
	if err != nil {
		if err == sql.ErrNoRows {
			return nilUser, ErrNoUsername
		}
		return nilUser, errors.Wrap(err, "failed to query for user")
	}

	if password == "" {
		return nilUser, ErrNoPassword
	}

	err = bcrypt.CompareHashAndPassword([]byte(hash), []byte(password))
	if err != nil {
		return nilUser, ErrBadPassword
	}

	return s.FindByUsername(username)
}

func (s *UserStore) FindByGitHubUsername(username string) (user core.User, err error) {
	rows, err := s.db.Query(fmt.Sprintf(
		"select %s from users where github_username = ?",
		strings.Join(userFields, ", ")),
		username)
	if err != nil {
		return user, errors.Wrap(err, "failed to query for user")
	}
	defer rows.Close()
	for rows.Next() {
		user, err := mapUser(rows)
		if err != nil {
			return user, errors.Wrap(err, "failed to read user")
		}
		return user, nil
	}
	return user, errors.New("username does not exist")
}

func (s *UserStore) FindByUsername(username string) (core.User, error) {
	user := core.User{}

	rows, err := s.db.Query(fmt.Sprintf(
		"select %s from users where username = ?",
		strings.Join(userFields, ", ")),
		username)
	if err != nil {
		return user, errors.Wrap(err, "failed to query for user")
	}
	defer rows.Close()
	for rows.Next() {
		user, err := mapUser(rows)
		if err != nil {
			return user, errors.Wrap(err, "failed to read user")
		}
		return user, nil
	}
	return user, errors.New("username does not exist")
}

func (s *UserStore) DeleteByUsername(username string) error {
	r, err := s.db.Exec("delete from users where username = ?",
		username)
	if err != nil {
		return err
	}
	n, err := r.RowsAffected()
	if err != nil {
		return err
	}
	if n == 0 {
		return ErrNoUsername
	}
	return nil
}

func mapUser(rows *sql.Rows) (core.User, error) {
	user := core.User{}

	var id string
	var sqlUsername sql.NullString
	var fullname sql.NullString
	var email sql.NullString
	var githubId sql.NullInt64
	var githubUsername sql.NullString

	err := rows.Scan(
		&id,
		&sqlUsername,
		&fullname,
		&email,
		&githubId,
		&githubUsername,
	)
	if err != nil {
		return user, err
	}

	user.Id = id

	if sqlUsername.Valid {
		user.Username = sqlUsername.String
	}
	if fullname.Valid {
		user.FullName = fullname.String
	}
	if email.Valid {
		user.Email = email.String
	}
	if githubId.Valid {
		user.GitHubID = githubId.Int64
	}
	if githubUsername.Valid {
		user.GitHubUsername = githubUsername.String
	}

	return user, nil
}

func (s *UserStore) FindAll() ([]core.User, error) {
	rows, err := s.db.Query(fmt.Sprintf("select %s from users",
		strings.Join(userFields, ", ")))
	if err != nil {
		return nil, errors.Wrap(err, "failed to query database")
	}
	defer rows.Close()

	users := make([]core.User, 0)

	for rows.Next() {
		user, err := mapUser(rows)
		if err != nil {
			return nil, errors.Wrap(err, "failed to read user")
		}
		users = append(users, user)
	}

	return users, nil
}

func (s *UserStore) UpdatePassword(username string, password string) error {
	hash, err := encryptPassword(password)
	if err != nil {
		return err
	}

	tx, err := s.db.Begin()
	if err != nil {
		return err
	}
	defer tx.Commit()

	r, err := tx.Exec(`update users set password = ? where username = ?`,
		hash, username)
	if err != nil {
		return err
	}
	n, err := r.RowsAffected()
	if err != nil {
		return err
	}
	if n == 0 {
		return ErrNoUsername
	}

	return nil
}

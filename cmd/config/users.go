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

package config

import (
	"bufio"
	"encoding/json"
	"fmt"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/server/auth"
	"github.com/jasonish/evebox/sqlite/configdb"
	"github.com/jasonish/evebox/util"
	"github.com/pkg/errors"
	"github.com/spf13/pflag"
	"golang.org/x/crypto/ssh/terminal"
	"net/http"
	"os"
	"strings"
	"syscall"
)

func UsersMain(db *configdb.ConfigDB, args []string) {
	usage := func() {
		fmt.Fprintf(os.Stderr, `Usage: users <command>

Commands:
    list
    add
    rm
    passwd

`)
	}

	if len(args) < 1 {
		usage()
		return
	}

	switch args[0] {
	case "list":
		UsersList(db, args[1:])
	case "add":
		UsersAdd(db, args[1:])
	case "rm":
		usersRemove(db, args[1:])
	case "passwd":
		usersPasswd(db, args[1:])
	default:
		usage()
	}
}

func fatal(msg string, args ...interface{}) {
	printerr(msg, args...)
	os.Exit(1)
}

func printerr(msg string, args ...interface{}) {
	fmt.Fprintf(os.Stderr, msg, args...)
	fmt.Fprintf(os.Stderr, "\n")
}

func println(msg string, args ...interface{}) {
	fmt.Printf(msg, args...)
	fmt.Printf("\n")
}

func readString(prompt string) string {
	reader := bufio.NewReader(os.Stdin)
	fmt.Printf("%s: ", prompt)
	response, err := reader.ReadString('\n')
	if err != nil {
		fatal("read error: %v", err)
	}
	return strings.TrimSpace(response)
}

func readPassword(prompt string) string {
	fmt.Printf("%s: ", prompt)
	password, err := terminal.ReadPassword(int(syscall.Stdin))
	if err != nil {
		fatal("read error: %v", err)
	}
	fmt.Printf("\n")
	return strings.TrimSpace(string(password))
}

func UsersAdd(db *configdb.ConfigDB, args []string) {
	var username string
	var password string
	var githubUsername string

	flagset := pflag.NewFlagSet("users add", pflag.ExitOnError)
	flagset.StringVarP(&username, "username", "u", "",
		"Username")
	flagset.StringVarP(&password, "password", "p", "",
		"Password")
	flagset.StringVar(&githubUsername, "github-username", "",
		"GitHub username (for Oauth2)")
	flagset.Parse(args)

	// Some validation.
	if password != "" && githubUsername != "" {
		fatal("error: password and external user-id may not be used together")
	}

	userstore := configdb.NewUserStore(db.DB)
	user := core.User{}

	if username == "" {
		username = readString("Enter username")
	}
	if checkForUsername(db, username) {
		fatal("error: username already exists.")
	}
	user.Username = username

	if githubUsername != "" {
		githubUser, err := getGitHubUser(githubUsername)
		if err != nil {
			fatal("Failed to get GitHub githubUser: %v", err)
		}
		println("Found GitHub githubUser: Username: %s; Email: %s; Name: %s.",
			githubUser.Login, githubUser.Email, githubUser.Name)
		r := readString("Add this githubUser [Y/n]")
		switch strings.ToLower(r) {
		case "", "y", "yes":
			break
		default:
			println("Exiting...")
			return
		}
		user.GitHubUsername = githubUser.Login
		user.GitHubID = githubUser.Id
		user.FullName = githubUser.Name
		user.Email = githubUser.Email
	} else if password == "" {
		for {
			password = readPassword("Enter password")
			confirm := readPassword("Re-enter password")
			if password == confirm {
				break
			}
			println("Passwords don't match, try again.")
		}
	}

	id, err := userstore.AddUser(user, password)
	if err != nil {
		fatal("Failed to add user: %v", err)
	}
	printerr("User added with ID %v", id)
}

func UsersList(db *configdb.ConfigDB, args []string) {
	userstore := configdb.NewUserStore(db.DB)

	users, err := userstore.FindAll()
	if err != nil {
		fatal("%v", err)
	}
	for _, user := range users {
		println("%s", util.ToJson(user))
	}
}

func checkForUsername(db *configdb.ConfigDB, username string) bool {
	userstore := configdb.NewUserStore(db.DB)
	_, err := userstore.FindByUsername(username)
	if err == nil {
		return true
	}
	return false
}

func usersRemove(db *configdb.ConfigDB, args []string) {
	var username string

	flagset := pflag.NewFlagSet("user-rm", pflag.ExitOnError)
	flagset.StringVarP(&username, "username", "u", "",
		"Username to remove")
	flagset.Parse(args)

	if username == "" {
		username = readString("Username to remove")
	}

	userstore := configdb.NewUserStore(db.DB)
	err := userstore.DeleteByUsername(username)
	if err != nil {
		printerr("Failed to delete user: %v", err)
		return
	}
	println("OK")
}

func getGitHubUser(username string) (user auth.GitHubUser, err error) {
	response, err := http.Get(fmt.Sprintf("%s/%s",
		"https://api.github.com/users",
		username))
	if err != nil {
		return user, err
	}
	if response.StatusCode != 200 {
		return user, errors.New(response.Status)
	}
	defer response.Body.Close()
	decoder := json.NewDecoder(response.Body)
	decoder.UseNumber()
	err = decoder.Decode(&user)
	if err != nil {
		return user, errors.Wrap(err, "failed to decode GitHub response")
	}
	return user, nil
}

func usersPasswd(db *configdb.ConfigDB, args []string) {
	var username string
	var password string

	var err error

	if len(args) > 0 {
		username = args[0]
	}
	if len(args) > 1 {
		password = args[1]
	}

	if username == "" {
		username = readString("Username")
	}

	if password == "" {
		for {
			password = readPassword("Password")
			confirm := readPassword("Confirm password")
			if password == confirm {
				break
			}
			println("Passwords don't match, try again.")
		}
	}

	userStore := configdb.NewUserStore(db.DB)

	err = userStore.UpdatePassword(username, password)
	if err != nil {
		fatal("Failed to update password: %v", err)
	}
}
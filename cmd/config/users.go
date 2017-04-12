package config

import (
	"bufio"
	"fmt"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/sqlite/configdb"
	"github.com/jasonish/evebox/util"
	"github.com/ogier/pflag"
	"golang.org/x/crypto/ssh/terminal"
	"os"
	"strings"
	"syscall"
)

func fatal(msg string, args ...interface{}) {
	printerr(msg, args)
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
	return strings.TrimSpace(string(password))
}

func UserAdd(db *configdb.ConfigDB, args []string) {
	userstore := configdb.NewUserStore(db.DB)

	username := readString("Enter username")
	password := readString("Enter password")
	printerr("")

	user := core.User{}
	user.Username = username
	id, err := userstore.AddUser(user, password)
	if err != nil {
		fatal("Failed to add user: %v", err)
	}
	printerr("User added with ID %v", id)
}

func UserList(db *configdb.ConfigDB, args []string) {
	userstore := configdb.NewUserStore(db.DB)

	users, err := userstore.FindAll()
	if err != nil {
		fatal("%v", err)
	}
	for _, user := range users {
		println("%s", util.ToJson(user))
	}
}

func UserRemove(db *configdb.ConfigDB, args []string) {
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

package config

import (
	"fmt"
	"github.com/jasonish/evebox/sqlite/configdb"
	"github.com/ogier/pflag"
	"log"
	"os"
)

func usage(flagset *pflag.FlagSet) {
	fmt.Fprintf(os.Stderr,
		"Usage: evebox config -D <dir> <command> [args...]\n")
	fmt.Fprintf(os.Stderr, "\n")
	fmt.Fprintf(os.Stderr, "Global options:\n")
	flagset.PrintDefaults()
	fmt.Fprintf(os.Stderr, `
Commands:
    user-list
    user-add
    user-rm
    user-passwd

`)
}

func Main(args []string) {

	var dataDirectory string

	flagset := pflag.NewFlagSet("evebox config", pflag.ExitOnError)
	flagset.Usage = func() {
		usage(flagset)
	}
	flagset.StringVarP(&dataDirectory, "data-directory", "D",
		"", "Data directory")
	flagset.Parse(args)

	commandArgs := flagset.Args()
	if len(commandArgs) == 0 {
		usage(flagset)
		os.Exit(1)
	}

	if dataDirectory == "" {
		log.Fatal("error: --data-directory is required")
	}

	db, err := configdb.NewConfigDB(dataDirectory)
	if err != nil {
		log.Fatalf("error: %v", err)
	}

	command := commandArgs[0]
	args = commandArgs[1:]
	switch command {
	case "user-add":
		UserAdd(db, args)
	case "user-list":
		UserList(db, args)
	default:
		fmt.Fprintf(os.Stderr, "error: unknown command: %s", command)
		os.Exit(1)
	}
}

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
    users

`)
}

func Main(args []string) {

	var dataDirectory string

	flagset := pflag.NewFlagSet("evebox config", pflag.ExitOnError)
	flagset.Usage = func() {
		usage(flagset)
	}
	flagset.SetInterspersed(false)
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
	case "users":
		UsersMain(db, args)
	default:
		fmt.Fprintf(os.Stderr, "error: unknown command: %s", command)
		os.Exit(1)
	}
}

/* Copyright (c) 2016 Jason Ish
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

package main

import (
	"fmt"
	"github.com/jasonish/evebox/cmd/agent"
	"github.com/jasonish/evebox/cmd/config"
	"github.com/jasonish/evebox/cmd/esimport"
	"github.com/jasonish/evebox/cmd/evereader"
	"github.com/jasonish/evebox/cmd/gencert"
	"github.com/jasonish/evebox/cmd/oneshot"
	"github.com/jasonish/evebox/cmd/pgimport"
	"github.com/jasonish/evebox/cmd/server"
	"github.com/jasonish/evebox/cmd/sqliteimport"
	"github.com/jasonish/evebox/core"
	"os"
	"runtime"
)

func VersionMain() {
	fmt.Printf("EveBox Version %s (rev %s); os=%s, arch=%s\n",
		core.BuildVersion, core.BuildRev, runtime.GOOS, runtime.GOARCH)
}

func Usage() {
	usage := fmt.Sprintf(`Usage: %s <command> [options]

Commands:
    server          Start the EveBox server
    agent           Start the EveBox agent
    config          Server configuration tool
    version         Print the EveBox version
    esimport        Run the Elastic Search Eve import tool
    evereader       Run the Eve log reader tool
    oneshot         Run one time with an eve.json file
    gencert         Generate TLS certificate

`, os.Args[0])
	fmt.Fprint(os.Stderr, usage)
}

func main() {

	if len(os.Args) == 1 || os.Args[1][0] == '-' {
		Usage()
		os.Exit(0)
	}

	switch os.Args[1] {
	case "version":
		VersionMain()
		return
	case "esimport":
		esimport.Main(os.Args[1:])
		return
	case "agent":
		agent.Main(os.Args[2:])
		return
	case "evereader":
		evereader.Main(os.Args[1:])
		return
	case "server":
		server.Main(os.Args[2:])
		return
	case "oneshot":
		oneshot.Main(os.Args[2:])
		return
	case "pgimport":
		pgimport.Main(os.Args[2:])
		return
	case "sqliteimport":
		sqliteimport.Main(os.Args[2:])
		return
	case "config":
		config.Main(os.Args[2:])
		return
	case "gencert":
		gencert.Main(os.Args[2:])
		return
	default:
		fmt.Fprintf(os.Stderr, "error: unknown command: %s\n", os.Args[1])
		os.Exit(1)
	}
}

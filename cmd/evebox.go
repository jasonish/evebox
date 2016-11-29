package main

import (
	"fmt"
	"github.com/jasonish/evebox/cmd/esimport"
	"github.com/jasonish/evebox/cmd/evereader"
	"github.com/jasonish/evebox/cmd/pgimport"
	"github.com/jasonish/evebox/cmd/server"
	"github.com/jasonish/evebox/cmd/sqliteimport"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/log"
	"os"
)

func VersionMain() {
	fmt.Printf("EveBox Version %s (rev %s) [%s]\n",
		core.BuildVersion, core.BuildRev, core.BuildDate)
}

func Usage() {
	usage := fmt.Sprintf(`Usage: %s <command> [options]

Commands:
	server			Start the EveBox server
	version			Print the EveBox version
	esimport		Run the Elastic Search Eve import tool
	evereader		Run the Eve log reader tool

`, os.Args[0])
	fmt.Fprint(os.Stderr, usage)
}

func main() {

	// Look for sub-commands, then fall back to server.
	if len(os.Args) > 1 && os.Args[1][0] != '-' {
		switch os.Args[1] {
		case "version":
			VersionMain()
			return
		case "esimport":
			esimport.Main(os.Args[1:])
			return
		case "evereader":
			evereader.Main(os.Args[1:])
			return
		case "server":
			server.Main(os.Args[2:])
			return
		case "pgimport":
			pgimport.Main(os.Args[2:])
			return
		case "sqliteimport":
			sqliteimport.Main(os.Args[2:])
			return
		default:
			log.Fatalf("Unknown command: %s", os.Args[1])
		}
	} else if len(os.Args) > 1 {
		switch os.Args[1] {
		case "-h":
			Usage()
			os.Exit(0)
		}
	}

	log.Info("No command provided, defaulting to server.")
	server.Main(os.Args[1:])
}

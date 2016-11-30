// +build !linux !amd64 !cgo

package sqliteimport

import "github.com/jasonish/evebox/log"

func Main(args []string) {
	log.Fatal("SQLite not supported in this build.")
}

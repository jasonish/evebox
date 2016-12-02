// +build !linux !amd64 !cgo

package sqlite

import "github.com/jasonish/evebox/core"
import "github.com/jasonish/evebox/log"

type DataStore struct {
	core.NotImplementedEventQueryService
	core.NotImplementedEventService
	core.NIAlertQueryService
}

func NewDataStore() (*DataStore, error) {
	log.Fatal("SQLite support not built in.")
	return nil, nil
}

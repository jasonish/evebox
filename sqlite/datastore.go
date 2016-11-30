package sqlite

import "github.com/jasonish/evebox/core"

type DataStore struct {
	*core.NotImplementedEventQueryService
}

func NewDataStore() *DataStore {
	return &DataStore{}
}

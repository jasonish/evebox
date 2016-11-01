package server

import (
	"github.com/jasonish/evebox/elasticsearch"
	"github.com/jasonish/evebox"
)

type AppContext struct {
	ElasticSearch *elasticsearch.ElasticSearch
	ArchiveService evebox.ArchiveService
}

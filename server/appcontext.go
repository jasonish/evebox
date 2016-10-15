package server

import "github.com/jasonish/evebox/elasticsearch"

type AppContext struct {
	ElasticSearch *elasticsearch.ElasticSearch
}

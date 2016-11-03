package server

import "net/http"

type ConfigResponse struct {
	ElasticSearchIndex string                   `json:"ElasticSearchIndex"`
	EventServices      []map[string]interface{} `json:"event-services"`
	Extra              map[string]interface{}   `json:"extra"`
}

func ConfigHandler(appContext AppContext, r *http.Request) interface{} {

	response := &ConfigResponse{}
	response.ElasticSearchIndex = appContext.Config.ElasticSearchIndex
	response.EventServices = appContext.Config.EventServices

	elasticSearchKeyword, _ := appContext.ElasticSearch.GetKeywordType("")
	response.Extra = map[string]interface{}{
		"elasticSearchKeyword": elasticSearchKeyword,
	}

	return response
}

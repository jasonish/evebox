package server

import (
	"fmt"
	"io/ioutil"
	"net/http"
)

// Elastic Search adapter handlers.

func EsBulkHandler(appcontent AppContext, r *http.Request) interface{} {
	response, err := appcontent.ElasticSearch.HttpClient.Post(
		fmt.Sprintf("_bulk?%s", r.URL.RawQuery),
		"application/json",
		r.Body)
	if err != nil {
		return err
	}

	bytes, err := ioutil.ReadAll(response.Body)
	if err != nil {
		return err
	}
	return bytes
}

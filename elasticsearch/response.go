package elasticsearch

import (
	"github.com/jasonish/evebox/util"
	"io"
	"io/ioutil"
	"encoding/json"
	"bytes"
)

type SearchResponse struct {
	Shards       map[string]interface{} `json:"_shards"`
	ScrollId     string                 `json:"_scroll_id"`
	TimedOut     bool                   `json:"timed_out"`
	Took         uint64                 `json:"took"`
	Hits         Hits                   `json:"hits"`
	Aggregations util.JsonMap           `json:"aggregations"`

	// A search may result in an error.
	Error  map[string]interface{} `json:"error"`
	Status int                    `json:"status"`

	Raw []byte
}

func DecodeSearchResponse(r io.Reader) (*SearchResponse, error) {

	raw, err := ioutil.ReadAll(r)
	if err != nil {
		return nil, err
	}

	decoder := json.NewDecoder(bytes.NewReader(raw))
	decoder.UseNumber()

	response := &SearchResponse{}
	if err := decoder.Decode(response); err != nil {
		return nil, err
	}

	response.Raw = raw

	return response, nil
}
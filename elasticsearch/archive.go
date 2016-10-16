package elasticsearch

import (
	"encoding/json"
	"net/http"
	"strings"

	"fmt"

	"io/ioutil"

	"github.com/jasonish/evebox/log"
)

type ExistsQuery struct {
	Field string
}

func (q ExistsQuery) MarshalJSON() ([]byte, error) {
	var object struct {
		Exists struct {
			Field string `json:"field"`
		} `json:"exists"`
	}
	object.Exists.Field = q.Field
	return json.Marshal(object)
}

type TermQuery struct {
	Field string
	Value interface{}
}

func (q TermQuery) MarshalJSON() ([]byte, error) {
	object := map[string]interface{}{
		"term": map[string]interface{}{
			q.Field: q.Value,
		},
	}
	return json.Marshal(object)
}

type RangeQuery struct {
	Field string
	Gte   string
	Lte   string
}

func (r RangeQuery) MarshalJSON() ([]byte, error) {
	values := map[string]string{}
	if r.Gte != "" {
		values["gte"] = r.Gte
	}
	if r.Lte != "" {
		values["lte"] = r.Lte
	}

	rangeq := map[string]interface{}{
		"range": map[string]interface{}{
			r.Field: values,
		},
	}

	return json.Marshal(rangeq)
}

// Wraps a search query in a {not: {...}}
type Not struct {
	Not interface{} `json:"not"`
}

type m map[string]interface{}

type l []interface{}

func ArchiveAlerts(es *ElasticSearch, signatureId uint64, srcIp string, destIp string,
	minTimestamp string, maxTimestamp string) error {

	query := m{
		"query": m{
			"bool": m{
				"filter": m{
					"and": l{
						ExistsQuery{"event_type"},
						TermQuery{"event_type", "alert"},
						RangeQuery{
							Field: "timestamp",
							Gte:   minTimestamp,
							Lte:   maxTimestamp,
						},
						TermQuery{"src_ip.raw", srcIp},
						TermQuery{"dest_ip.raw", destIp},
						TermQuery{
							"alert.signature_id",
							signatureId,
						},
						Not{TermQuery{"tags", "archived"}},
					},
				},
			},
		},
		"_source": "tags",
		"sort": l{
			"_doc",
		},
		"size": 10000,
	}

	path := fmt.Sprintf("%s/_search?scroll=1m", es.EventIndex)
	log.Println(path)
	response, err := es.PostJson(path, query)
	if err != nil {
		log.Error("Failed to initialize scroll: %v", err)
		return err
	}
	decoder := json.NewDecoder(response.Body)
	searchResponse := SearchResponse{}
	decoder.Decode(&searchResponse)

	scrollId := searchResponse.ScrollId

	for {

		if len(searchResponse.Hits.Hits) == 0 {
			log.Info("No more events to archive.")
			break
		}

		log.Info("Found %d events to archive.", len(searchResponse.Hits.Hits))
		err = BulkAddTags(es, searchResponse.Hits.Hits,
			[]string{"evebox.archived", "archived"})
		if err != nil {
			log.Error("BulkAddTags failed: %v", err)
			return err
		}

		// Get next set of events to archive.

		path := "_search/scroll"
		body := m{
			"scroll":    "1m",
			"scroll_id": scrollId,
		}
		response, err := es.PostJson(path, body)
		if err != nil {
			log.Error("Failed to fetch from scroll: %v", err)
			return err
		}

		decoder := json.NewDecoder(response.Body)
		searchResponse = SearchResponse{}
		decoder.Decode(&searchResponse)
	}

	response, err = es.DeleteWithStringBody("_search/scroll",
		"application/json", scrollId)
	if err != nil {
		log.Error("Failed to delete scroll id: %v", err)
	}
	deleteResponse, err := ioutil.ReadAll(response.Body)
	if err != nil {
		log.Error("Failed to read delete response: %v", err)
	}
	log.Info("Delete response: %s", string(deleteResponse))

	log.Info("Sending refresh.")
	response, err = es.PostString("_refresh", "application/json", "{}")
	if err != nil {
		log.Error("Failed to post refresh: %v", err)
		return err
	}
	body, err := ioutil.ReadAll(response.Body)
	if err != nil {
		log.Error("Failed to read response body: %v", err)
	}
	log.Println(string(body))

	return nil

	//for {
	//	path := fmt.Sprintf("%s/log/_search?scroll=1m", es.EventIndex)
	//	log.Println(path)
	//	response, err := es.PostJson(path, query)
	//	if err != nil {
	//		log.Error("es.PostJson failed: %v", err)
	//		return err
	//	}
	//
	//	decoder := json.NewDecoder(response.Body)
	//	searchResponse = SearchResponse{}
	//	decoder.Decode(&searchResponse)
	//	log.Println(ToJson(searchResponse))
	//
	//	if searchResponse.Hits.Total == 0 {
	//		log.Info("Found 0 events, stopping archive.")
	//		break
	//	}
	//
	//	log.Info("Found %d events to archive", searchResponse.Hits.Total)
	//	err = BulkAddTags(es, searchResponse.Hits.Hits,
	//		[]string{"evebox.archived", "archived"})
	//	if err != nil {
	//		log.Error("BulkAddTags failed: %v", err)
	//		return err
	//	}
	//
	//}

	return nil
}

func BulkAddTags(es *ElasticSearch, documents []map[string]interface{}, _tags []string) error {
	bulk := make([]string, 0)

	for _, item := range documents {
		doc := JsonMap(item)

		// Add tags.
		tags := make([]string, 0)

		itags, ok := doc.GetMap("_source").Get("tags").([]interface{})
		if ok {
			for _, tag := range itags {
				tags = append(tags, tag.(string))
			}
		}
		for _, tag := range _tags {
			if !StringSliceContains(tags, tag) {
				tags = append(tags, tag)
			}
		}

		id := doc.Get("_id").(string)
		docType := doc.Get("_type").(string)
		index := doc.Get("_index").(string)

		command := m{
			"update": m{
				"_id":    id,
				"_type":  docType,
				"_index": index,
			},
		}
		bulk = append(bulk, ToJson(command))

		partial := m{
			"doc": m{
				"tags": tags,
			},
		}
		bulk = append(bulk, ToJson(partial))
	}

	// Needs to finish with a new line.
	bulk = append(bulk, "")
	bulkString := strings.Join(bulk, "\n")
	response, err := es.PostString("_bulk", "application/json", bulkString)
	if err != nil {
		log.Error("Failed to archive events: %v", err)
		return err
	}

	if response.StatusCode != http.StatusOK {
		return NewElasticSearchError(response)
	} else {
		bulkResponse := BulkResponse{}
		decoder := json.NewDecoder(response.Body)
		decoder.UseNumber()
		err = decoder.Decode(&bulkResponse)
		if err != nil {
			log.Error("Failed to decode bulk response: %v", err)
		} else {
			log.Info("Archived %d events; errors=%v",
				len(bulkResponse.Items), bulkResponse.Errors)
			if bulkResponse.Errors {
				for _, item := range bulkResponse.Items {
					logBulkError(item)
				}
			}
		}
	}

	return nil
}

func logBulkError(item map[string]interface{}) {
	update, ok := item["update"].(map[string]interface{})
	if !ok || update == nil {
		return
	}
	error := update["error"]
	if error == nil {
		return
	}
	log.Notice("Archive error: %s", ToJson(error))
}

package elasticsearch

import (
	"encoding/json"
	"net/http"
	"strings"

	"io/ioutil"

	"io"

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

type m map[string]interface{}

type l []interface{}

func ArchiveAlerts(es *ElasticSearch, signatureId uint64, srcIp string, destIp string,
	minTimestamp string, maxTimestamp string) error {

	query := m{
		"query": m{
			"bool": m{
				"filter": l{
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
				},
				"must_not": l{
					TermQuery{"tags", "archived"},
				},
			},
		},
		"_source": "tags",
		"sort": l{
			"_doc",
		},
		"size": 10000,
	}

	searchResponse, err := es.SearchScroll(query, "1m")
	if err != nil {
		log.Error("Failed to initialize scroll: %v", err)
		return err
	}

	scrollId := searchResponse.ScrollId

	for {

		log.Debug("Search response total: %d; hits: %d",
			searchResponse.Hits.Total, len(searchResponse.Hits.Hits))

		if len(searchResponse.Hits.Hits) == 0 {
			break
		}

		// We do this in a retry loop as some documents may fail to be
		// updated. Most likely rejected due to max thread count or
		// something.
		maxRetries := 5
		retries := 0
		for {
			retry, err := BulkAddTags(es, searchResponse.Hits.Hits,
				[]string{"evebox.archived", "archived"})
			if err != nil {
				log.Error("BulkAddTags failed: %v", err)
				return err
			}
			if !retry {
				break
			}
			retries++
			if retries > maxRetries {
				log.Warning("Errors occurred archive events, not all events may have been archived.")
				break
			}
		}

		// Get next set of events to archive.
		searchResponse, err = es.Scroll(scrollId, "1m")
		if err != nil {
			log.Error("Failed to fetch from scroll: %v", err)
			return err
		}

	}

	response, err := es.DeleteWithStringBody("_search/scroll",
		"application/json", scrollId)
	if err != nil {
		log.Error("Failed to delete scroll id: %v", err)
	}
	io.Copy(ioutil.Discard, response.Body)

	response, err = es.PostString("_refresh", "application/json", "{}")
	if err != nil {
		log.Error("Failed to post refresh: %v", err)
		return err
	}
	io.Copy(ioutil.Discard, response.Body)

	return nil
}

func BulkAddTags(es *ElasticSearch, documents []map[string]interface{}, _tags []string) (bool, error) {
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
		return false, err
	}

	retry := false

	if response.StatusCode != http.StatusOK {
		return retry, NewElasticSearchError(response)
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
				retry = true
				for _, item := range bulkResponse.Items {
					logBulkError(item)
				}
			}
		}
	}

	return retry, nil
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

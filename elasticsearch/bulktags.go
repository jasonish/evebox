/* Copyright (c) 2016 Jason Ish
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 *
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED ``AS IS'' AND ANY EXPRESS OR IMPLIED
 * WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * DISCLAIMED. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY DIRECT,
 * INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
 * (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING
 * IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
 * POSSIBILITY OF SUCH DAMAGE.
 */

package elasticsearch

import (
	"github.com/jasonish/evebox/log"
	"net/http"
	"strings"
)

// bulkAddTags will add a set of tags to a set of documents using the Elastic
// Search bulk API.
func bulkAddTags(es *ElasticSearch, documents []map[string]interface{},
	_tags []string) (bool, error) {

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
	response, err := es.HttpClient.PostString("_bulk", "application/json", bulkString)
	if err != nil {
		log.Error("Failed to archive events: %v", err)
		return false, err
	}

	retry := false

	if response.StatusCode != http.StatusOK {
		return retry, NewElasticSearchError(response)
	} else {
		bulkResponse := BulkResponse{}

		if err := es.Decode(response, &bulkResponse); err != nil {
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

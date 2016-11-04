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
	"fmt"
	"github.com/jasonish/evebox/log"
	"io"
	"io/ioutil"
)

type ArchiveService struct {
	es      *ElasticSearch
	keyword string
}

func NewArchiveService(es *ElasticSearch) *ArchiveService {
	archiveService := &ArchiveService{
		es: es,
	}

	keyword, err := es.GetKeywordType("")
	if err != nil {
		log.Warning("Failed to determine Elastic Search keyword type, using 'keyword'")
		archiveService.keyword = keyword
	} else {
		log.Info("Using Elastic Search keyword type %s.", keyword)
		archiveService.keyword = keyword
	}

	return archiveService
}

func (s *ArchiveService) KeywordTermQuery(term string, value string) TermQuery {
	return TermQuery{
		fmt.Sprintf("%s.%s", term, s.keyword),
		value,
	}
}

func (s *ArchiveService) ArchiveAlerts(signatureId uint64,
	srcIp string, destIp string,
	minTimestamp string, maxTimestamp string) error {

	query := m{
		"query": m{
			"bool": m{
				"filter": l{
					ExistsQuery("event_type"),
					s.KeywordTermQuery("event_type", "alert"),
					RangeQuery{
						Field: "timestamp",
						Gte:   minTimestamp,
						Lte:   maxTimestamp,
					},
					s.KeywordTermQuery("src_ip", srcIp),
					s.KeywordTermQuery("dest_ip", destIp),
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

	searchResponse, err := s.es.SearchScroll(query, "1m")
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
			retry, err := bulkAddTags(s.es, searchResponse.Hits.Hits,
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
		searchResponse, err = s.es.Scroll(scrollId, "1m")
		if err != nil {
			log.Error("Failed to fetch from scroll: %v", err)
			return err
		}

	}

	response, err := s.es.DeleteScroll(scrollId)
	if err != nil {
		log.Error("Failed to delete scroll id: %v", err)
	}
	io.Copy(ioutil.Discard, response.Body)

	response, err = s.es.HttpClient.PostString("_refresh", "application/json", "{}")
	if err != nil {
		log.Error("Failed to post refresh: %v", err)
		return err
	}
	io.Copy(ioutil.Discard, response.Body)

	return nil
}

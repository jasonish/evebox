package elasticsearch

import (
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/log"
	"io"
	"io/ioutil"
)

// EventService methods for manipulating tags on events.

// ArchiveAlertGroup is a specialization of AddTagsToAlertGroup.
func (s *EventService) ArchiveAlertGroup(p core.AlertGroupQueryParams) error {
	return s.AddTagsToAlertGroup(p, []string{"archived", "evebox.archived"})
}

// EscalateAlertGroup is a specialization of AddTagsToAlertGroup.
func (s *EventService) EscalateAlertGroup(p core.AlertGroupQueryParams) error {
	return s.AddTagsToAlertGroup(p, []string{"escalated", "evebox.escalated"})
}

func (s *EventService) AddTagsToAlertGroup(p core.AlertGroupQueryParams, tags []string) error {

	mustNot := []interface{}{}
	for _, tag := range tags {
		mustNot = append(mustNot, TermQuery("tags", tag))
	}

	query := m{
		"query": m{
			"bool": m{
				"filter": l{
					ExistsQuery("event_type"),
					KeywordTermQuery("event_type", "alert", s.keyword),
					RangeQuery{
						Field: "timestamp",
						Gte:   p.MinTimestamp,
						Lte:   p.MaxTimestamp,
					},
					KeywordTermQuery("src_ip", p.SrcIP, s.keyword),
					KeywordTermQuery("dest_ip", p.DstIP, s.keyword),
					TermQuery("alert.signature_id", p.SignatureID),
				},
				"must_not": mustNot,
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
				tags)
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

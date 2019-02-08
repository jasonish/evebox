/* Copyright (c) 2016-2017 Jason Ish
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
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"github.com/pkg/errors"
	"time"
)

type DataStore struct {
	es *ElasticSearch
	core.UnimplementedDatastore
}

func NewDataStore(es *ElasticSearch) (*DataStore, error) {
	datastore := DataStore{
		es: es,
	}
	return &datastore, nil
}

func (d *DataStore) GetEveEventSink() core.EveEventSink {
	return NewIndexer(d.es)
}

func (s *DataStore) asKeyword(keyword string) string {
	return s.es.FormatKeyword(keyword)
}

// FindFlow finds the flow events matching the query parameters in options.
func (d *DataStore) FindFlow(flowId uint64, proto string, timestamp string,
	srcIp string, destIp string) (interface{}, error) {

	query := NewEventQuery()

	query.EventType("flow")
	query.AddFilter(TermQuery("flow_id", flowId))
	query.AddFilter(TermQuery("proto", proto))
	query.AddFilter(RangeLte("flow.start", timestamp))
	query.AddFilter(RangeGte("flow.end", timestamp))
	query.ShouldHaveIp(srcIp, d.es.GetKeyword())
	query.ShouldHaveIp(destIp, d.es.GetKeyword())

	response, err := d.es.Search(query)
	if err != nil {
		log.Error("%v", err)
		return nil, err
	}

	return response.Hits.Hits, nil
}

// FindNetflow finds netflow events matching the parameters in options.
func (s *DataStore) FindNetflow(options core.EventQueryOptions, sortBy string,
	order string) (interface{}, error) {

	size := int64(10)

	if options.Size > 0 {
		size = options.Size
	}

	if order == "" {
		order = "desc"
	}

	query := NewEventQuery()
	query.AddFilter(TermQuery("event_type", "netflow"))

	if options.TimeRange != "" {
		query.AddTimeRangeFilter(options.TimeRange)
	}

	if options.QueryString != "" {
		query.AddFilter(QueryString(options.QueryString))
	}

	if sortBy != "" {
		query.Aggs["agg"] = TopHitsAgg(sortBy, order, size)
	} else {
		query.SetSize(size)
	}

	response, err := s.es.Search(query)
	if err != nil {
		return nil, err
	}

	// Unwrap response.
	hits := response.Aggregations.GetMap("agg").GetMap("hits").Get("hits")

	return map[string]interface{}{
		"data": hits,
	}, nil
}

const ACTION_ARCHIVED = "archived"

const ACTION_ESCALATED = "escalated"

const ACTION_DEESCALATED = "de-escalated"

const ACTION_COMMENT = "comment"

type HistoryEntry struct {
	Timestamp string `json:"timestamp"`
	Username  string `json:"username"`
	Action    string `json:"action"`
	Comment   string `json:"comment,omitempty"`
}

func (s *DataStore) buildAlertGroupQuery(p core.AlertGroupQueryParams) *EventQuery {
	q := EventQuery{}
	q.AddFilter(ExistsQuery("event_type"))
	q.AddFilter(KeywordTermQuery("event_type", "alert", s.es.GetKeyword()))
	q.AddFilter(NewRangeQuery("@timestamp", eve.FormatTimestampUTC(p.MinTs),
		eve.FormatTimestampUTC(p.MaxTs)))
	q.AddFilter(KeywordTermQuery("src_ip", p.SrcIP, s.es.GetKeyword()))
	q.AddFilter(KeywordTermQuery("dest_ip", p.DstIP, s.es.GetKeyword()))
	q.AddFilter(TermQuery("alert.signature_id", p.SignatureID))
	return &q
}

// ArchiveAlertGroupByQuery uses the Elastic Search update_by_query API to
// archive events with a query instead of updating each document. This is
// only available in Elastic Search v5+.
func (s *DataStore) AddTagsToAlertGroupsByQuery(p core.AlertGroupQueryParams, tags []string, action HistoryEntry) error {
	var mustNot []interface{}
	for _, tag := range tags {
		mustNot = append(mustNot, TermQuery("tags", tag))
	}

	query := s.buildAlertGroupQuery(p)
	if len(mustNot) > 0 {
		query.Query.Bool.MustNot = mustNot
	}
	query.Script = &Script{
		Lang: "painless",
		Inline: `
		        if (params.tags != null) {
			        if (ctx._source.tags == null) {
			            ctx._source.tags = new ArrayList();
			        }
			        for (tag in params.tags) {
			            if (!ctx._source.tags.contains(tag)) {
			                ctx._source.tags.add(tag);
			            }
			        }
			    }
			    if (ctx._source.evebox == null) {
			        ctx._source.evebox = new HashMap();
			    }
			    if (ctx._source.evebox.history == null) {
			        ctx._source.evebox.history = new ArrayList();
			    }
			    ctx._source.evebox.history.add(params.action);
		`,
		Params: map[string]interface{}{
			"tags":   tags,
			"action": action,
		},
	}

	response, err := s.es.doUpdateByQuery(query)
	if err != nil {
		log.Error("failed to update by query: %v", err)
		return err
	}
	failures := response.GetMapList("failures")
	log.Info("Events updated: %v; failures=%d {%+v}",
		response.Get("updated"), len(failures), failures)

	return nil
}

// ArchiveAlertGroup is a specialization of AddTagsToAlertGroup.
func (s *DataStore) ArchiveAlertGroup(p core.AlertGroupQueryParams, user core.User) error {
	tags := []string{"archived", "evebox.archived"}
	return s.AddTagsToAlertGroupsByQuery(p, tags, HistoryEntry{
		Action:    ACTION_ARCHIVED,
		Timestamp: FormatTimestampUTC(time.Now()),
		Username:  user.Username,
	})
}

// EscalateAlertGroup is a specialization of AddTagsToAlertGroup.
func (s *DataStore) EscalateAlertGroup(p core.AlertGroupQueryParams, user core.User) error {
	tags := []string{"escalated", "evebox.escalated"}
	history := HistoryEntry{
		Username:  user.Username,
		Action:    ACTION_ESCALATED,
		Timestamp: FormatTimestampUTC(time.Now()),
	}
	return s.AddTagsToAlertGroupsByQuery(p, tags, history)
}

func (s *DataStore) DeEscalateAlertGroup(p core.AlertGroupQueryParams, user core.User) error {
	tags := []string{"escalated", "evebox.escalated"}
	return s.RemoveTagsFromAlertGroupsByQuery(p, tags, HistoryEntry{
		Username:  user.Username,
		Timestamp: FormatTimestampUTC(time.Now()),
		Action:    ACTION_DEESCALATED,
	})
}

func (s *DataStore) RemoveTagsFromAlertGroupsByQuery(p core.AlertGroupQueryParams,
	tags []string, action HistoryEntry) error {
	var should []interface{}
	for _, tag := range tags {
		should = append(should, TermQuery("tags", tag))
	}

	query := s.buildAlertGroupQuery(p)
	query.Query.Bool.Should = should
	query.Script = &Script{
		Lang: "painless",
		Inline: `
			    if (ctx._source.tags != null) {
			        for (tag in params.tags) {
			            ctx._source.tags.removeIf(entry -> entry == tag);
			        }
			    }
			    if (ctx._source.evebox == null) {
			        ctx._source.evebox = new HashMap();
			    }
			    if (ctx._source.evebox.history == null) {
			        ctx._source.evebox.history = new ArrayList();
			    }
			    ctx._source.evebox.history.add(params.action);
		`,
		Params: map[string]interface{}{
			"tags":   tags,
			"action": action,
		},
	}

	response, err := s.es.doUpdateByQuery(query)
	if err != nil {
		log.Error("failed to update by query: %v", err)
		return err
	}
	failures := response.GetMapList("failures")
	log.Info("Events updated: %v; failures=%d {%+v}",
		response.Get("updated"), len(failures), failures)

	return nil
}

func (s *DataStore) CommentOnAlertGroup(p core.AlertGroupQueryParams, user core.User, comment string) error {
	history := HistoryEntry{
		Username:  user.Username,
		Action:    ACTION_COMMENT,
		Comment:   comment,
		Timestamp: FormatTimestampUTC(time.Now()),
	}
	return s.AddTagsToAlertGroupsByQuery(p, nil, history)
}

func (s *DataStore) CommentOnEventId(eventId string, user core.User, comment string) error {

	event, err := s.GetEventById(eventId)
	if err != nil {
		return errors.Wrapf(err, "failed to find event with ID %s", eventId)
	}
	doc := Document{event}

	action := HistoryEntry{
		Username:  user.Username,
		Action:    ACTION_COMMENT,
		Comment:   comment,
		Timestamp: FormatTimestampUTC(time.Now()),
	}

	query := EventQuery{}
	query.Script = &Script{
		Lang: "painless",
		Inline: `
			    if (ctx._source.evebox == null) {
			        ctx._source.evebox = new HashMap();
			    }
			    if (ctx._source.evebox.history == null) {
			        ctx._source.evebox.history = new ArrayList();
			    }
			    ctx._source.evebox.history.add(params.action);
		`,
		Params: map[string]interface{}{
			"action": action,
		},
	}

	_, err = s.es.Update(doc.Index(), doc.Type(), doc.Id(), query)
	return err
}

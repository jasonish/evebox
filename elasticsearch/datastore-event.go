/* Copyright (c) 2014-2017 Jason Ish
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
	"github.com/jasonish/evebox/log"
	"github.com/pkg/errors"
	"time"
)

// ArchiveEvent archives an individual event by ID.
func (s *DataStore) ArchiveEvent(eventId string, user core.User) error {
	event, err := s.GetEventById(eventId)
	if err != nil {
		return errors.Wrap(err, "failed to get event")
	}
	if event == nil {
		return core.NewEventNotFoundError(eventId)
	}
	eventDoc := Document{event}

	request := map[string]interface{}{
		"script": map[string]interface{}{
			"lang": "painless",
			"inline": `
			    if (ctx._source.tags == null) {
			        ctx._source.tags = new ArrayList();
			    }
			    for (tag in params.tags) {
			        if (!ctx._source.tags.contains(tag)) {
			            ctx._source.tags.add(tag);
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
			"params": map[string]interface{}{
				"tags": []string{"archived", "evebox.archived"},
				"action": HistoryEntry{
					Action:    ACTION_ARCHIVED,
					Timestamp: FormatTimestampUTC(time.Now()),
					Username:  user.Username,
				},
			},
		},
	}

	_, err = s.es.Update(eventDoc.Index(), eventDoc.Id(), request)
	if err != nil {
		log.Error("update error: %v", err)
		return err
	}

	return nil
}

// EscalateEvent escalated an individual event by ID.
func (s *DataStore) EscalateEvent(eventId string, user core.User) error {
	event, err := s.GetEventById(eventId)
	if err != nil {
		return errors.Wrap(err, "failed to get event")
	}
	eventDoc := Document{event}

	request := map[string]interface{}{
		"script": map[string]interface{}{
			"lang": "painless",
			"inline": `
			    if (ctx._source.tags == null) {
			        ctx._source.tags = new ArrayList();
			    }
			    for (tag in params.tags) {
			        if (!ctx._source.tags.contains(tag)) {
			            ctx._source.tags.add(tag);
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
			"params": map[string]interface{}{
				"tags": []string{"escalated", "evebox.escalated"},
				"action": HistoryEntry{
					Action:    ACTION_ESCALATED,
					Timestamp: FormatTimestampUTC(time.Now()),
					Username:  user.Username,
				},
			},
		},
	}

	_, err = s.es.Update(eventDoc.Index(), eventDoc.Id(), request)
	if err != nil {
		log.Error("update error: %v", err)
		return err
	}

	return nil
}

// DeEscalateEvent de-escalates an individual event by ID.
func (s *DataStore) DeEscalateEvent(eventId string, user core.User) error {
	event, err := s.GetEventById(eventId)
	if err != nil {
		return errors.Wrap(err, "failed to get event")
	}
	eventDoc := Document{event}

	request := map[string]interface{}{
		"script": map[string]interface{}{
			"lang": "painless",
			"inline": `
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
			"params": map[string]interface{}{
				"tags": []string{"escalated", "evebox.escalated"},
				"action": HistoryEntry{
					Action:    ACTION_DEESCALATED,
					Timestamp: FormatTimestampUTC(time.Now()),
					Username:  user.Username,
				},
			},
		},
	}

	_, err = s.es.Update(eventDoc.Index(), eventDoc.Id(), request)
	if err != nil {
		log.Error("update error: %v", err)
		return err
	}

	return nil
}

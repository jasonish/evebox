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

import "github.com/jasonish/evebox/util"

// AddTagsToEvent will add the given tags to the event referenced by ID.
func (s *DataStore) AddTagsToEvent(id string, addTags []string) error {

	raw, err := s.GetEventById(id)
	if err != nil {
		return err
	}

	event := util.JsonMap(raw)
	tags := event.GetMap("_source").GetAsStrings("tags")

	for _, tag := range addTags {
		if !util.StringSliceContains(tags, tag) {
			tags = append(tags, tag)
		}
	}

	s.es.PartialUpdate(event.GetString("_index"), event.GetString("_type"),
		event.GetString("_id"), map[string]interface{}{
			"tags": tags,
		})

	return nil
}

// RemoveTagsFromEvent will remove the given tags from the event referenced
// by ID.
func (s *DataStore) RemoveTagsFromEvent(id string, rmTags []string) error {

	raw, err := s.GetEventById(id)
	if err != nil {
		return err
	}

	event := util.JsonMap(raw)
	currentTags := event.GetMap("_source").GetAsStrings("tags")
	tags := make([]string, 0)

	for _, tag := range currentTags {
		if !util.StringSliceContains(rmTags, tag) {
			tags = append(tags, tag)
		}
	}

	s.es.PartialUpdate(event.GetString("_index"), event.GetString("_type"),
		event.GetString("_id"), map[string]interface{}{
			"tags": tags,
		})

	return nil
}

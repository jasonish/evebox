/* Copyright (c) 2017 Jason Ish
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

// Event operations.

package postgres

import (
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/elasticsearch"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/util"
	"time"
)

func (d *PgDatastore) ArchiveEvent(eventId string, user core.User) error {
	sqlTemplate := `update events
set
  archived = true,
  metadata = jsonb_set(
    metadata,
    '{"history"}',
    case when metadata->'history' is null then '[]'::jsonb
      else metadata->'history' end || $2::jsonb
    )
where
uuid = $1`

	history := elasticsearch.HistoryEntry{
		Action:    elasticsearch.ACTION_ARCHIVED,
		Username:  user.Username,
		Timestamp: eve.FormatTimestampUTC(time.Now()),
	}

	start := time.Now()
	_, err := d.pg.Exec(sqlTemplate, eventId, util.ToJson(history))
	log.Info("Archive event took %v", time.Now().Sub(start))
	return err
}

func (d *PgDatastore) EscalateEvent(eventId string, user core.User) error {
	sqlTemplate := `update events
set
  escalated = true,
  metadata = jsonb_set(
    metadata,
    '{"history"}',
    case when metadata->'history' is null then '[]'::jsonb
      else metadata->'history' end || $2::jsonb
    )
where
uuid = $1
and escalated = false`

	history := elasticsearch.HistoryEntry{
		Action:    elasticsearch.ACTION_ESCALATED,
		Username:  user.Username,
		Timestamp: eve.FormatTimestampUTC(time.Now()),
	}

	_, err := d.pg.Exec(sqlTemplate, eventId, util.ToJson(history))
	return err
}

func (d *PgDatastore) DeEscalateEvent(eventId string, user core.User) error {
	sqlTemplate := `update events
set
  escalated = false,
    metadata = jsonb_set(
    metadata,
    '{"history"}',
    case when metadata->'history' is null then '[]'::jsonb
      else metadata->'history' end || $2::jsonb
    )
where
uuid = $1
and escalated = true`

	history := elasticsearch.HistoryEntry{
		Action:    elasticsearch.ACTION_DEESCALATED,
		Username:  user.Username,
		Timestamp: eve.FormatTimestampUTC(time.Now()),
	}

	_, err := d.pg.Exec(sqlTemplate, eventId, util.ToJson(history))
	return err
}

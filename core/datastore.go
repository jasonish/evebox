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

package core

import (
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
)

// EventConsumer takes events and stores them in a data store.
type EveEventConsumer interface {
	// Submit takes an event for submission to the datastore.
	Submit(event eve.EveEvent) error

	// Commit commits, or flushes out the event to the datastore. In some
	// cases this might be a no-op.
	Commit() (status interface{}, err error)
}

type Datastore interface {
	EventService

	GetEveEventConsumer() EveEventConsumer

	AlertQuery(options AlertQueryOptions) (interface{}, error)
	EventQuery(options EventQueryOptions) (interface{}, error)

	ArchiveAlertGroup(p AlertGroupQueryParams) error
	EscalateAlertGroup(p AlertGroupQueryParams) error
	UnstarAlertGroup(p AlertGroupQueryParams) error

	GetEventById(id string) (map[string]interface{}, error)
	FindFlow(flowId uint64, proto string, timestamp string, srcIp string, destIp string) (interface{}, error)
}

type UnimplementedDatastore struct {
}

func (d *UnimplementedDatastore) GetEveEventConsumer() EveEventConsumer {
	log.Warning("GetEventConsumer not implemented in this datastore")
	return nil
}

func (s *UnimplementedDatastore) AlertQuery(options AlertQueryOptions) (interface{}, error) {
	log.Warning("AlertQuery not implemented in this datastore")
	return nil, NotImplementedError
}

func (s *UnimplementedDatastore) EventQuery(options EventQueryOptions) (interface{}, error) {
	log.Warning("EventQuery not implemented in this datastore")
	return nil, NotImplementedError
}

func (s *UnimplementedDatastore) UnstarAlertGroup(p AlertGroupQueryParams) error {
	log.Warning("UnstarAlertGroup not implemented in this datastore")
	return NotImplementedError
}

func (s *UnimplementedDatastore) GetEventById(id string) (map[string]interface{}, error) {
	log.Warning("GetEventById not implement by this datastore")
	return nil, NotImplementedError
}

func (s *UnimplementedDatastore) FindFlow(flowId uint64, proto string, timestamp string, srcIp string, destIp string) (interface{}, error) {
	return nil, NotImplementedError
}

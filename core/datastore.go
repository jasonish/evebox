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
	"fmt"
	"github.com/jasonish/evebox/log"
	"github.com/pkg/errors"
)

var NotImplementedError error

func init() {
	NotImplementedError = fmt.Errorf("Not implemented.")
}

type Datastore interface {
	GetEveEventSink() EveEventSink
	AlertQuery(options AlertQueryOptions) ([]AlertGroup, error)
	EventQuery(options EventQueryOptions) (interface{}, error)
	ArchiveAlertGroup(p AlertGroupQueryParams, u User) error
	EscalateAlertGroup(p AlertGroupQueryParams, u User) error
	DeEscalateAlertGroup(p AlertGroupQueryParams, u User) error
	ArchiveEvent(eventId string, user User) error
	EscalateEvent(eventId string, user User) error
	DeEscalateEvent(eventId string, user User) error
	GetEventById(id string) (map[string]interface{}, error)
	FindFlow(flowId uint64, proto string, timestamp string, srcIp string, destIp string) (interface{}, error)
	FindNetflow(options EventQueryOptions, sortBy string, order string) (interface{}, error)
	CommentOnEventId(eventId string, user User, comment string) error
	CommentOnAlertGroup(p AlertGroupQueryParams, user User, comment string) error
	FlowHistogram(options FlowHistogramOptions) (interface{}, error)
}

type UnimplementedDatastore struct {
}

func (d *UnimplementedDatastore) CommentOnAlertGroup(p AlertGroupQueryParams, user User, comment string) error {
	return errors.New("CommentOnAlertGroup not implemented by active datastore.")
}

func (d *UnimplementedDatastore) CommentOnEventId(eventId string, user User, comment string) error {
	return errors.New("CommentOnEventId not implemented by active datastore.")
}

func (d *UnimplementedDatastore) ArchiveEvent(eventId string, user User) error {
	return errors.New("ArchiveEvent not implemented by this datastore.")
}

func (d *UnimplementedDatastore) EscalateEvent(eventId string, user User) error {
	return errors.New("EscalateEvent not implemented by this datastore.")
}

func (d *UnimplementedDatastore) DeEscalateEvent(eventId string, user User) error {
	return errors.New("DeEscalateEvent not implemented by this datastore.")
}

func (d *UnimplementedDatastore) GetEveEventSink() EveEventSink {
	log.Warning("GetEveEventSink not implemented by this datastore.")
	return nil
}

func (s *UnimplementedDatastore) AlertQuery(options AlertQueryOptions) ([]AlertGroup, error) {
	log.Warning("AlertQuery not implemented in this datastore")
	return nil, NotImplementedError
}

func (s *UnimplementedDatastore) EventQuery(options EventQueryOptions) (interface{}, error) {
	log.Warning("EventQuery not implemented in this datastore")
	return nil, NotImplementedError
}

func (s *UnimplementedDatastore) DeEscalateAlertGroup(p AlertGroupQueryParams, u User) error {
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

func (s *UnimplementedDatastore) ArchiveAlertGroup(p AlertGroupQueryParams, u User) error {
	return NotImplementedError
}

func (s *UnimplementedDatastore) EscalateAlertGroup(p AlertGroupQueryParams, u User) error {
	return NotImplementedError
}

func (s *UnimplementedDatastore) FindNetflow(options EventQueryOptions, sortBy string, order string) (interface{}, error) {
	return nil, NotImplementedError
}

func (s *UnimplementedDatastore) FlowHistogram(options FlowHistogramOptions) (interface{}, error) {
	return nil, NotImplementedError
}


/* Copyright (c) 2014-2015 Jason Ish
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

// AlertGroupQueryParams holds the parameters for querying a specific
// group of alerts.
type AlertGroupQueryParams struct {
	SignatureID  uint64
	SrcIP        string
	DstIP        string
	MinTimestamp string
	MaxTimestamp string
}

// AlertQueryOptions includes the options for querying alerts which are then
// returned as alert groups.
type AlertQueryOptions struct {
	// Tags that events must have.
	MustHaveTags []string

	// Tags that events must not have.
	MustNotHaveTags []string

	// Query string.
	QueryString string

	// Time range which is a duration string telling how far back
	// to log for events (eg. 24h, 1m). Must be parsable by
	// https://golang.org/pkg/time/#ParseDuration
	TimeRange string
}

type AlertQueryService interface {
	Query(options AlertQueryOptions) (interface{}, error)
}

type EventQueryOptions struct {
	QueryString string

	Size int64

	// Maximum timestamp to include in result set.
	MaxTs string

	// Minimum timestamp to include in result set.
	MinTs string

	// Event type to limit results to.
	EventType string
}

type EventQueryService interface {
	Query(options EventQueryOptions) (interface{}, error)
}

type EventService interface {
	GetEventById(id string) (map[string]interface{}, error)

	ArchiveAlertGroup(p AlertGroupQueryParams) error

	EscalateAlertGroup(p AlertGroupQueryParams) error

	AddTagsToAlertGroup(p AlertGroupQueryParams, tags []string) error

	RemoveTagsFromAlertGroup(p AlertGroupQueryParams, tags []string) error

	AddTagsToEvent(id string, tags []string) error

	RemoveTagsFromEvent(id string, tags []string) error
}

type ReportOptions struct {
	Size int64

	QueryString string

	TimeRange string

	// Limit the result set to events with this address as either the
	// source or the destination.
	AddressFilter string

	// Limit results to a certain event type.
	EventType string

	// Subtypes...
	DnsType string

	// Limit results to a specific sensor name.
	SensorFilter string
}

type ReportService interface {
	ReportDnsRequestRrnames(options ReportOptions) (interface{}, error)

	// Report on alert aggregations. For example the top "alert.signature"s
	// with their counts. Valid aggregations include:
	// - src_ip
	// - dest_ip
	// - alert.category
	// - alert.signature
	// - src_port
	// - dest_port
	ReportAlertAggs(agg string, options ReportOptions) (interface{}, error)

	ReportHistogram(interval string, options ReportOptions) (interface{}, error)
}

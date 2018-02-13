package core

import "time"

type FlowHistogramOptions struct {
	MinTs       time.Time
	MaxTs       time.Time
	TimeRange   string
	Interval    string
	SubAggs     []string
	QueryString string
}

type FlowService interface {
	Histogram(options FlowHistogramOptions) interface{}
}

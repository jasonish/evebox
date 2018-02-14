package core

type FlowHistogramOptions struct {
	CommonQueryOptions
	Interval    string
	SubAggs     []string
}

type FlowService interface {
	Histogram(options FlowHistogramOptions) interface{}
}

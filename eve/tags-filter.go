package eve

// TagsFilter is an Eve filter to ensure that the event has a tags list/array.
type TagsFilter struct {
}

func (f *TagsFilter) Filter(event RawEveEvent) {
	if event["tags"] == nil {
		event["tags"] = []interface{}{}
	}
}

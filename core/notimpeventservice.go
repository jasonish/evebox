package core

import "fmt"

var NotImplementedError error

func init() {
	NotImplementedError = fmt.Errorf("Not implemented.")
}

// NotImplementedEventService is an implementation of core.EventService with
// every function created but returning an not implemented error.
type NotImplementedEventService struct {
}

func (s *NotImplementedEventService) GetEventById(id string) (map[string]interface{}, error) {
	return nil, NotImplementedError
}

func (s *NotImplementedEventService) AddTagsToEvent(id string, tags []string) error {
	return NotImplementedError
}

func (s *NotImplementedEventService) AddTagsToAlertGroup(p AlertGroupQueryParams, tags []string) error {
	return NotImplementedError
}

func (s *NotImplementedEventService) RemoveTagsFromAlertGroup(p AlertGroupQueryParams, tags []string) error {
	return NotImplementedError
}

func (s *NotImplementedEventService) RemoveTagsFromEvent(id string, tags []string) error {
	return NotImplementedError
}

func (s *NotImplementedEventService) ArchiveAlertGroup(p AlertGroupQueryParams) error {
	return NotImplementedError
}

func (s *NotImplementedEventService) EscalateAlertGroup(p AlertGroupQueryParams) error {
	return NotImplementedError
}

func (s *NotImplementedEventService) FindNetflow(options EventQueryOptions, sortBy string, order string) (interface{}, error) {
	return nil, NotImplementedError
}

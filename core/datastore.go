package core

type Datastore interface {
	EventQueryService
	AlertQueryService
	EventService
}

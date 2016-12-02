package elasticsearch

type DataStore struct {
	*AlertQueryService
	*EventQueryService
	*EventService

	es *ElasticSearch
}

func NewDataStore(es *ElasticSearch) (*DataStore, error) {

	alertQueryService := NewAlertQueryService(es)
	eventQueryService := NewEventQueryService(es)
	eventService := NewEventService(es)

	datastore := DataStore{
		AlertQueryService: alertQueryService,
		EventQueryService: eventQueryService,
		EventService:      eventService,
		es:                es,
	}

	return &datastore, nil
}

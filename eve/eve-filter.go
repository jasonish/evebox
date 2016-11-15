package eve

type EveFilter interface {
	Filter(event RawEveEvent)
}

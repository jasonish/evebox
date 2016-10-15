package elasticsearch

// A wrapper around a generic string map for accessing elements.
type JsonMap map[string]interface{}

func (m JsonMap) GetMap(name string) JsonMap {
	return JsonMap(m[name].(map[string]interface{}))
}

func (m JsonMap) Get(name string) interface{} {
	return m[name]
}

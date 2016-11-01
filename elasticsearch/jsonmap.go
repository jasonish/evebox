package elasticsearch

// A wrapper around a generic string map for accessing elements.
type JsonMap map[string]interface{}

func (m JsonMap) GetMap(name string) JsonMap {
	if m == nil {
		return nil
	}
	val := m[name]
	if val != nil {
		return val.(map[string]interface{})
	}
	return nil
}

func (m JsonMap) GetMapList(name string) []JsonMap {
	if m == nil {
		return nil
	}

	switch v := m[name].(type) {
	case []interface{}:
		result := make([]JsonMap, 0)
		for _, item := range v {
			result = append(result, JsonMap(item.(map[string]interface{})))
		}
		return result
	}

	return nil
}

func (m JsonMap) Get(name string) interface{} {
	if m == nil {
		return nil
	}
	return m[name]
}

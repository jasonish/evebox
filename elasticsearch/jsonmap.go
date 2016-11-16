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

func (m JsonMap) GetString(name string) string {
	if m == nil {
		return ""
	}
	if m[name] == nil {
		return ""
	}
	val, ok := m[name].(string)
	if !ok {
		return ""
	}
	return val
}

// GetAsStrings will return the value with the given name as a slice
// of strings. On failure an empty slice will be returned.
func (m JsonMap) GetAsStrings(name string) []string {
	if m[name] == nil {
		return []string{}
	}
	items, ok := m[name].([]interface{})
	if !ok {
		return []string{}
	}
	strings := make([]string, 0, len(items))
	if items != nil {
		for _, item := range items {
			strings = append(strings, item.(string))
		}

	}
	return strings
}

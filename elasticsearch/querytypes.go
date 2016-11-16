package elasticsearch

import "fmt"

func ExistsQuery(field string) interface{} {
	return map[string]interface{}{
		"exists": map[string]interface{}{
			"field": field,
		},
	}
}

func TermQuery(field string, value interface{}) map[string]interface{} {
	return map[string]interface{}{
		"term": map[string]interface{}{
			field: value,
		},
	}
}

func KeywordTermQuery(field string, value string, suffix string) map[string]interface{} {
	return TermQuery(fmt.Sprintf("%s.%s", field, suffix), value)
}

func QueryString(query string) map[string]interface{} {
	return map[string]interface{}{
		"query_string": map[string]interface{}{
			"query": query,
		},
	}
}

func Sort(field string, order string) map[string]interface{} {
	return map[string]interface{}{
		field: map[string]interface{}{
			"order": order,
		},
	}
}

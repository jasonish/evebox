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

func PrefixQuery(field string, value interface{}) map[string]interface{} {
	return map[string]interface{}{
		"prefix": map[string]interface{}{
			field: value,
		},
	}
}

func KeywordTermQuery(field string, value string, suffix string) map[string]interface{} {
	return TermQuery(fmt.Sprintf("%s.%s", field, suffix), value)
}

func KeywordPrefixQuery(field string, value string, suffix string) map[string]interface{} {
	return PrefixQuery(fmt.Sprintf("%s.%s", field, suffix), value)
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

func Range(rangeType string, field string, value interface{}) interface{} {
	return map[string]interface{}{
		"range": map[string]interface{}{
			field: map[string]interface{}{
				rangeType: value,
			},
		},
	}
}

func RangeGte(field string, value interface{}) interface{} {
	return Range("gte", field, value)
}

func RangeLte(field string, value interface{}) interface{} {
	return Range("lte", field, value)
}

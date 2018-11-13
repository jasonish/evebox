package elasticsearch

import (
	"github.com/stretchr/testify/assert"
	"testing"
)

func TestNewRangeQuery(t *testing.T) {

	// No values set.
	query := NewRangeQuery("@timestamp", nil, nil)
	expected := map[string]interface{}{
		"range": map[string]interface{}{
			"@timestamp": map[string]interface{}{},
		},
	}
	assert.Equal(t, query, expected)

	// Gte set.
	query = NewRangeQuery("@timestamp", 100, nil)
	expected = map[string]interface{}{
		"range": map[string]interface{}{
			"@timestamp": map[string]interface{}{
				"gte": 100,
			},
		},
	}
	assert.Equal(t, query, expected)

	// Lte set.
	query = NewRangeQuery("@timestamp", nil, 200)
	expected = map[string]interface{}{
		"range": map[string]interface{}{
			"@timestamp": map[string]interface{}{
				"lte": 200,
			},
		},
	}
	assert.Equal(t, query, expected)

	// Both gte and lte set.
	query = NewRangeQuery("@timestamp", 100, 200)
	expected = map[string]interface{}{
		"range": map[string]interface{}{
			"@timestamp": map[string]interface{}{
				"gte": 100,
				"lte": 200,
			},
		},
	}
	assert.Equal(t, query, expected)
}

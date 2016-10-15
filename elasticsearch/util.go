package elasticsearch

import (
	"encoding/json"
	"fmt"
)

func ToJson(value interface{}) string {
	buf, err := json.Marshal(value)
	if err != nil {
		return fmt.Sprintf("<failed to marshal to json: %v>", err)
	}
	return string(buf)
}

// Check if a slice of strings contains a string.
func StringSliceContains(slice []string, what string) bool {
	for _, item := range slice {
		if item == what {
			return true
		}
	}
	return false
}

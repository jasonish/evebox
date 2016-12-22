package util

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

func ToJsonPretty(value interface{}) string {
	buf, err := json.MarshalIndent(value, "", "    ")
	if err != nil {
		return fmt.Sprintf("<failed to marshal to json: %v>", err)
	}
	return string(buf)
}

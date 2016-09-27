package eve

import "time"

const EveTimestampFormat = "2006-01-02T15:04:05.999999999Z0700"

// A RawEveEvent is an Eve event decoded into map[string]interface{} which
// contains all the data in its raw format.
type RawEveEvent map[string]interface{}

func (e RawEveEvent) GetTimestamp() (*time.Time, error) {
	result, err := time.Parse(EveTimestampFormat, e["timestamp"].(string))
	if err != nil {
		return nil, err
	}
	return &result, nil
}

package sqlite

import "time"

// Format an Eve timestamp as an SQLite timestamp.
func eveTs2SqliteTs(timestamp string) (string, error) {
	var RFC3339Nano_Modified string = "2006-01-02T15:04:05.999999999Z0700"
	result, err := time.Parse(RFC3339Nano_Modified, timestamp)
	if err != nil {
		return "", err
	}
	return result.UTC().Format("2006-01-02T15:04:05.999999Z"), nil
}

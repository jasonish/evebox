package geoip

import (
	"testing"
	"log"
	"encoding/json"
)

func TestGeoIp(t *testing.T) {

	path := FindDbPath()
	if path == "" {
		t.Skip("Failed to find GeoIP database.")
	}

	db, err := NewGeoIpDb("")
	if err != nil {
		t.Fatal(err)
	}

	result, err := db.LookupString("149.56.128.130")
	if err != nil {
		t.Fatal(err)
	}
	asJson, err := json.Marshal(result)
	if err != nil {
		t.Fatal(err)
	}
	log.Println(string(asJson))
}
package geoip

import (
	"os"
	"fmt"
	"github.com/oschwald/geoip2-golang"
	"time"
	"net"
)

const (
	CITY = "GeoLite2-City"
	COUNTRY = "Geolite2-Country"
)

var DbLocations = []string{
	"/usr/local/share/GeoIP/GeoLite2-City.mmdb",
	"/usr/share/GeoIP/GeoLite2-City.mmdb",
	"/usr/local/share/GeoIP/GeoLite2-Country.mmdb",
	"/usr/share/GeoIP/GeoLite2-Country.mmdb",
}

func FindDbPath() (string) {
	for _, path := range DbLocations {
		if _, err := os.Stat(path); err == nil {
			return path
		}
	}
	return ""
}

type GeoIp struct {
	Ip            string `json:"ip"`
	ContinentCode string `json:"continent_code"`
	CountryCode2  string `json:"country_code2"`
	CountryName   string `json:"country_name"`
	RegionName    string `json:"region_name"`
	RegionCode    string `json:"region_code"`
	CityName      string `json:"city_name"`
	Latitude      float64 `json:"latitude"`
	Longitude     float64 `json:"longitude"`
	Coordinates   [2]float64 `json:"coordinates"`
}

type GeoIpDb struct {
	reader    *geoip2.Reader
	buildDate time.Time
	dbType    string
}

func NewGeoIpDb(path string) (*GeoIpDb, error) {

	if path == "" {
		path = FindDbPath()
		if path == "" {
			return nil, fmt.Errorf("no database files found")
		}
	}

	reader, err := geoip2.Open(path)
	if err != nil {
		return nil, err
	}

	dbType := reader.Metadata().DatabaseType
	switch dbType {
	case CITY:
	case COUNTRY:
	default:
		return nil, fmt.Errorf("Unsupported database type: %s", dbType)
	}

	buildDate := time.Unix(int64(reader.Metadata().BuildEpoch), 0)

	return &GeoIpDb{
		reader: reader,
		buildDate: buildDate,
		dbType: dbType,
	}, nil
}

func (g *GeoIpDb) Type() string {
	return g.dbType
}

func (g *GeoIpDb) BuildDate() time.Time {
	return g.buildDate
}

func (g *GeoIpDb) LookupString(addr string) (GeoIp, error) {
	ip := net.ParseIP(addr)

	result := GeoIp{
		Ip: addr,
	}

	if g.dbType == "GeoLite2-Country" {
		country, err := g.reader.Country(ip)
		if err != nil {
			return result, err
		}
		result.ContinentCode = country.Continent.Code
		result.CountryCode2 = country.Country.IsoCode
		result.CountryName = country.Country.Names["en"]
	} else if g.dbType == "GeoLite2-City" {
		city, err := g.reader.City(ip)
		if err != nil {
			return result, err
		}
		result.ContinentCode = city.Continent.Code
		result.CountryCode2 = city.Country.IsoCode
		result.CountryName = city.Country.Names["en"]
		if len(city.Subdivisions) > 0 {
			result.RegionCode = city.Subdivisions[0].IsoCode
			result.RegionName = city.Subdivisions[0].Names["en"]
		}

		result.Latitude = city.Location.Latitude
		result.Longitude = city.Location.Longitude

		// Coordinates are [longtitude, latitude].
		result.Coordinates[0] = city.Location.Longitude
		result.Coordinates[1] = city.Location.Latitude
	}

	return result, nil
}
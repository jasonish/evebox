/* Copyright (c) 2016 Jason Ish
 * All rights reserved.
 *
 * Redistribution and use in source and binary forms, with or without
 * modification, are permitted provided that the following conditions
 * are met:
 *
 * 1. Redistributions of source code must retain the above copyright
 *    notice, this list of conditions and the following disclaimer.
 * 2. Redistributions in binary form must reproduce the above copyright
 *    notice, this list of conditions and the following disclaimer in the
 *    documentation and/or other materials provided with the distribution.
 *
 * THIS SOFTWARE IS PROVIDED ``AS IS'' AND ANY EXPRESS OR IMPLIED
 * WARRANTIES, INCLUDING, BUT NOT LIMITED TO, THE IMPLIED WARRANTIES OF
 * MERCHANTABILITY AND FITNESS FOR A PARTICULAR PURPOSE ARE
 * DISCLAIMED. IN NO EVENT SHALL THE AUTHOR BE LIABLE FOR ANY DIRECT,
 * INDIRECT, INCIDENTAL, SPECIAL, EXEMPLARY, OR CONSEQUENTIAL DAMAGES
 * (INCLUDING, BUT NOT LIMITED TO, PROCUREMENT OF SUBSTITUTE GOODS OR
 * SERVICES; LOSS OF USE, DATA, OR PROFITS; OR BUSINESS INTERRUPTION)
 * HOWEVER CAUSED AND ON ANY THEORY OF LIABILITY, WHETHER IN CONTRACT,
 * STRICT LIABILITY, OR TORT (INCLUDING NEGLIGENCE OR OTHERWISE) ARISING
 * IN ANY WAY OUT OF THE USE OF THIS SOFTWARE, EVEN IF ADVISED OF THE
 * POSSIBILITY OF SUCH DAMAGE.
 */

package geoip

import (
	"compress/gzip"
	"fmt"
	"github.com/oschwald/geoip2-golang"
	"github.com/pkg/errors"
	"io/ioutil"
	"net"
	"os"
	"strings"
	"time"
)

const (
	CITY    = "GeoLite2-City"
	COUNTRY = "Geolite2-Country"
)

var DbLocations = []string{
	"/etc/evebox/GeoLite2-City.mmdb.gz",
	"/etc/evebox/GeoLite2-City.mmdb",
	"/usr/local/share/GeoIP/GeoLite2-City.mmdb",
	"/usr/share/GeoIP/GeoLite2-City.mmdb",
}

func FindDbPath() string {
	for _, path := range DbLocations {
		if _, err := os.Stat(path); err == nil {
			return path
		}
	}
	return ""
}

func OpenDb(path string) (*geoip2.Reader, error) {
	if strings.HasSuffix(path, ".gz") {
		file, err := os.Open(path)
		if err != nil {
			return nil, errors.Errorf(
				"Failed to open geoip database: %s: %v", path, err)
		}
		gzipReader, err := gzip.NewReader(file)
		if err != nil {
			return nil, errors.Errorf(
				"Failed to open geoip database: %s: %v", path, err)
		}
		bytes, err := ioutil.ReadAll(gzipReader)
		if err != nil {
			return nil, errors.Errorf(
				"Failed to open geoip database: %s: %v", path, err)
		}
		return geoip2.FromBytes(bytes)
	}

	return geoip2.Open(path)
}

type GeoIp struct {
	Ip            string     `json:"ip,omitempty"`
	Ip6           string     `json:"ip6,omitempty"`
	ContinentCode string     `json:"continent_code,omitempty"`
	CountryCode2  string     `json:"country_code2,omitempty"`
	CountryName   string     `json:"country_name,omitempty"`
	RegionName    string     `json:"region_name,omitempty"`
	RegionCode    string     `json:"region_code,omitempty"`
	CityName      string     `json:"city_name,omitempty"`
	Latitude      float64    `json:"latitude,omitempty"`
	Longitude     float64    `json:"longitude,omitempty"`
	Coordinates   [2]float64 `json:"coordinates,omitempty"`
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

	reader, err := OpenDb(path)
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
		reader:    reader,
		buildDate: buildDate,
		dbType:    dbType,
	}, nil
}

func (g *GeoIpDb) Type() string {
	return g.dbType
}

func (g *GeoIpDb) BuildDate() time.Time {
	return g.buildDate
}

func (g *GeoIpDb) LookupString(addr string) (*GeoIp, error) {
	ip := net.ParseIP(addr)

	result := GeoIp{}

	// Logstash/elasticsearch template work-around - the template for
	// logstash expects IPv4 addresses in the geoip section, so for now
	// put IPv6 addresses in the "ip6" field.
	if ip.To4() != nil {
		result.Ip = addr
	} else {
		result.Ip6 = addr
	}

	if g.dbType == "GeoLite2-Country" {
		country, err := g.reader.Country(ip)
		if err != nil {
			return nil, err
		}
		result.ContinentCode = country.Continent.Code
		result.CountryCode2 = country.Country.IsoCode
		result.CountryName = country.Country.Names["en"]
	} else if g.dbType == "GeoLite2-City" {
		city, err := g.reader.City(ip)
		if err != nil {
			return nil, err
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

	return &result, nil
}

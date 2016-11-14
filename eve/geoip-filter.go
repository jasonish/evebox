package eve

import (
	"net"
	"github.com/jasonish/evebox/geoip"
	"github.com/jasonish/evebox/log"
)

var RFC1918_Netstrings = []string{
	"10.0.0.0/8",
	"127.16.0.0/12",
	"192.168.0.0/16",
}

var RFC1918_IPNets []*net.IPNet

func IsRFC1918(addr string) bool {
	ip := net.ParseIP(addr)
	for _, ipnet := range RFC1918_IPNets {
		if ipnet.Contains(ip) {
			return true
		}
	}
	return false
}

func init() {
	for _, network := range RFC1918_Netstrings {
		_, ipnet, err := net.ParseCIDR(network)
		if err == nil {
			RFC1918_IPNets = append(RFC1918_IPNets, ipnet)
		}
	}
}

type GeoipFilter struct {
	db *geoip.GeoIpDb
}

func NewGeoipFilter(db *geoip.GeoIpDb) *GeoipFilter {
	return &GeoipFilter{
		db: db,
	}
}

func (f *GeoipFilter) AddGeoIP(event RawEveEvent) {

	if f.db == nil {
		return
	}

	srcip, ok := event["src_ip"].(string)
	if ok && !IsRFC1918(srcip) {
		gip, err := f.db.LookupString(srcip)
		if err != nil {
			log.Debug("Failed to lookup geoip for %s", srcip)
		}

		// Need at least a continent code.
		if gip.ContinentCode != "" {
			event["geoip"] = gip
		}
	}
	if event["geoip"] == nil {
		destip, ok := event["dest_ip"].(string)
		if ok && !IsRFC1918(destip) {
			gip, err := f.db.LookupString(destip)
			if err != nil {
				log.Debug("Failed to lookup geoip for %s", destip)
			}
			// Need at least a continent code.
			if gip.ContinentCode != "" {
				event["geoip"] = gip
			}
		}
	}

}
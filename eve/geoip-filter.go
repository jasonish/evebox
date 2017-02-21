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

package eve

import (
	"github.com/jasonish/evebox/geoip"
	"github.com/jasonish/evebox/log"
	"net"
)

var rfc1918_Netstrings = []string{
	"10.0.0.0/8",
	"127.16.0.0/12",
	"192.168.0.0/16",
}

var rfc1918_IPNets []*net.IPNet

func init() {
	for _, network := range rfc1918_Netstrings {
		_, ipnet, err := net.ParseCIDR(network)
		if err == nil {
			rfc1918_IPNets = append(rfc1918_IPNets, ipnet)
		}
	}
}

func isRFC1918(addr string) bool {
	ip := net.ParseIP(addr)

	for _, ipnet := range rfc1918_IPNets {
		if ipnet.Contains(ip) {
			return true
		}
	}

	return false
}

type GeoipFilter struct {
	service *geoip.GeoIpService
}

func NewGeoipFilter(service *geoip.GeoIpService) *GeoipFilter {
	return &GeoipFilter{
		service: service,
	}
}

func (f *GeoipFilter) Filter(event EveEvent) {

	if f.service == nil {
		return
	}

	if event["geoip"] != nil {
		return
	}

	srcip, ok := event["src_ip"].(string)
	if ok && !isRFC1918(srcip) {
		gip, err := f.service.LookupString(srcip)
		if err != nil {
			log.Debug("Failed to lookup geoip for %s", srcip)
		}
		if gip != nil {
			event["geoip"] = gip
		}
	}

	if event["geoip"] == nil {
		destip, ok := event["dest_ip"].(string)
		if ok && !isRFC1918(destip) {
			gip, err := f.service.LookupString(destip)
			if err != nil {
				log.Debug("Failed to lookup geoip for %s", destip)
			}
			if gip != nil {
				event["geoip"] = gip
			}
		}
	}

}

/* Copyright (c) 2014-2015 Jason Ish
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

package appcontext

import (
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/elasticsearch"
	"github.com/jasonish/evebox/geoip"
	"github.com/jasonish/evebox/sqlite/configdb"
)

type GithubAuthConfig struct {
	Enabled      bool
	ClientID     string
	ClientSecret string
	Callback     string
}

type Config struct {
	Http struct {
		TlsEnabled     bool
		TlsCertificate string
		TlsKey         string
		ReverseProxy   bool
		RequestLogging bool
	}

	LetsEncryptHostname string

	Authentication struct {
		Required bool

		// Username or Usernamepassword.
		Type string

		LoginMessage string

		// GitHub Oauth2.
		Github GithubAuthConfig
	}
}

type AppContext struct {
	// Configuration data that is not held in the configuration database.
	Config Config

	ConfigDB  *configdb.ConfigDB
	Userstore core.UserStore

	// The interface to the underlying datastore.
	DataStore core.Datastore

	ElasticSearch *elasticsearch.ElasticSearch

	ReportService core.ReportService

	GeoIpService *geoip.GeoIpService

	Features map[core.Feature]bool

	// A default time range to send to a client. Mainly useful for oneshot
	// server mode where we want to set a better time range.
	DefaultTimeRange string

	// Tell the client to ignore any locally stored configuration of the
	// default time range.
	ForceDefaultTimeRange bool
}

func (c *AppContext) SetFeature(feature core.Feature) {
	if c.Features == nil {
		c.Features = map[core.Feature]bool{}
	}
	c.Features[feature] = true
}

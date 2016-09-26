/* Copyright (c) 2014-2016 Jason Ish
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

package main

import (
	"crypto/tls"
	"log"
	"net"
	"net/http"
	"net/http/httputil"
	"net/url"
	"os"
)

type ElasticSearchProxy struct {
	proxy            *httputil.ReverseProxy
	prefix           string
	disableCertCheck bool
}

var EsProxyLogger *log.Logger

func NewElasticSearchProxy(elasticSearchUrl string, prefix string,
	disableCertCheck bool) (*ElasticSearchProxy, error) {

	esUrl, err := url.Parse(elasticSearchUrl)
	if err != nil {
		return nil, err
	}
	proxy := ElasticSearchProxy{
		proxy:            httputil.NewSingleHostReverseProxy(esUrl),
		prefix:           prefix,
		disableCertCheck: disableCertCheck,
	}

	EsProxyLogger = log.New(os.Stderr, "elasticsearch-proxy: ", 0)

	proxy.proxy.ErrorLog = EsProxyLogger

	proxy.proxy.Transport = &http.Transport{DialTLS: proxy.DialTLS}

	return &proxy, nil
}

func (p *ElasticSearchProxy) ServeHTTP(w http.ResponseWriter, r *http.Request) {

	// Strip the prefix from the URL.
	r.URL.Path = r.URL.Path[len(p.prefix):]

	// Strip headers that will get in the way of CORS.
	r.Header.Del("X-Forwarded-For")
	r.Header.Del("Origin")
	r.Header.Del("Referer")

	p.proxy.ServeHTTP(w, r)
}

// As most users are likely to be using a self-signed certificate on their
// Elastic Search install, set InsecureSkipVerify by default.
func (p *ElasticSearchProxy) DialTLS(network string, addr string) (net.Conn, error) {
	return tls.Dial(network, addr, &tls.Config{
		InsecureSkipVerify: p.disableCertCheck,
	})
}

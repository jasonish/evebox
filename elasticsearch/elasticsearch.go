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

package elasticsearch

import (
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"strings"

	"bytes"
	"github.com/jasonish/evebox/httpclient"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/util"
	"github.com/pkg/errors"
	"io/ioutil"
	"math"
	"time"
)

const LOG_REQUEST_RESPONSE = false

type Config struct {
	BaseURL          string
	DisableCertCheck bool
	Username         string
	Password         string
	Template         string
	Index            string
	KeywordSuffix    string
	NoKeywordSuffix  bool
	ForceTemplate    bool
	DocType          string
}

type ElasticSearch struct {
	// User provided config.
	config Config

	baseUrl  string
	username string
	password string

	EventSearchIndex string
	EventIndexPrefix string

	// These are filled on a ping request. Perhaps when creating an
	// instance of this object we should ping.
	VersionString string
	MajorVersion  int64
	MinorVersion  int64

	useIpDatatype bool

	httpClient *httpclient.HttpClient
}

func New(config Config) *ElasticSearch {
	url := strings.TrimSuffix(config.BaseURL, "/")

	httpClient := httpclient.NewHttpClient()
	httpClient.SetBaseUrl(url)
	httpClient.DisableCertCheck(config.DisableCertCheck)
	httpClient.SetUsernamePassword(config.Username, config.Password)

	es := &ElasticSearch{
		config:     config,
		baseUrl:    url,
		httpClient: httpClient,
	}

	es.setEventIndex(config.Index)

	return es
}

func (es *ElasticSearch) setEventIndex(index string) {
	if strings.HasSuffix(index, "*") {
		baseIndex := strings.TrimSuffix(strings.TrimSuffix(index, "*"), "-")
		es.EventSearchIndex = index
		es.EventIndexPrefix = baseIndex
	} else {
		es.EventIndexPrefix = index
		es.EventSearchIndex = fmt.Sprintf("%s-*", index)
	}
	log.Info("Event base index: %s", es.EventIndexPrefix)
	log.Info("Event search index: %s", es.EventSearchIndex)
}

func (es *ElasticSearch) Decode(response *http.Response, v interface{}) error {
	body, err := ioutil.ReadAll(response.Body)
	if err != nil {
		return err
	}
	log.Debug("Decoding response (truncated at 1024 bytes): %s",
		string(body)[:int(math.Min(1024, float64(len(body))))])
	decoder := json.NewDecoder(bytes.NewReader(body))
	decoder.UseNumber()
	return decoder.Decode(v)
}

// GetKeyword returns the Elastic Search keyword suffix for searches on
// unanalyzed terms.
func (es *ElasticSearch) GetKeyword() string {
	return es.config.KeywordSuffix
}

func (es *ElasticSearch) Ping() (*PingResponse, error) {
	response, err := es.httpClient.Get("")
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()

	if response.StatusCode != 200 {
		return nil, DecodeResponseAsError(response)
	}

	body, err := DecodeResponse(response)
	if err != nil {
		return nil, err
	}

	pingResponse := PingResponse{body}
	es.MajorVersion, es.MinorVersion = pingResponse.ParseVersion()
	return &pingResponse, nil
}

func (es *ElasticSearch) GetTemplate(name string) (util.JsonMap, error) {
	url := fmt.Sprintf("_template/%s", name)
	log.Debug("Fetching template %s", url)
	response, err := es.httpClient.Get(url)
	if err != nil {
		return nil, err
	}

	template := util.JsonMap{}

	if err := es.Decode(response, &template); err != nil {
		return nil, err
	}

	return template, nil
}

func (es *ElasticSearch) GetUseIpDatatype() bool {
	return es.useIpDatatype
}

func (es *ElasticSearch) ConfigureIndex() error {
	index := es.EventIndexPrefix

	// Use a template name matching the index prefix unless explicitly set.
	templateName := index
	if es.config.Template != "" {
		templateName = es.config.Template
	}

	if strings.HasPrefix(index, "filebeat") {
		if es.config.KeywordSuffix == "" && !es.config.NoKeywordSuffix {
			es.config.KeywordSuffix = ""
			es.config.NoKeywordSuffix = true
		}
		return nil
	}

	template, err := es.GetTemplate(templateName)
	if err != nil {
		log.Warning("Failed to get template from Elastic Search, keyword resolution delayed.")
		return err
	}

	keys := template.GetKeys()
	log.Info("Found templates %s", keys)

	version := template.GetMap(index).Get("version")
	log.Debug("Found template version %v", version)

	// Check if we should use the IP datatype on src_ip and dest_ip.
	properties := template.GetMap(index).
		GetMap("mappings").GetMap("_default_").GetMap("properties")
	if properties != nil {
		destIpType := properties.GetMap("dest_ip").GetString("type")
		sourceIpType := properties.GetMap("src_ip").GetString("type")
		if destIpType == "ip" && sourceIpType == "ip" {
			log.Info("Elastic Search EVE records are using IP datatype.")
			es.useIpDatatype = true
		}
	}

	// Determine keyword.
	if !es.config.NoKeywordSuffix {
		keywordFound := false
		dynamicTemplates := template.GetMap(index).
			GetMap("mappings").
			GetMap("_default_").
			GetMapList("dynamic_templates")
		for _, entry := range dynamicTemplates {
			if entry["string_fields"] != nil {
				mappingType := entry.GetMap("string_fields").
					GetMap("mapping").
					GetMap("fields").
					GetMap("keyword").
					Get("type")
				if mappingType == "keyword" {
					es.config.KeywordSuffix = "keyword"
					keywordFound = true
				}
			}
		}

		if !keywordFound {
			dynamicTemplates := template.GetMap(index).
				GetMap("mappings").
				GetMapList("dynamic_templates")
			for _, entry := range dynamicTemplates {
				if entry["string_fields"] != nil {
					mappingType := entry.GetMap("string_fields").
						GetMap("mapping").
						GetMap("fields").
						GetMap("keyword").
						Get("type")
					if mappingType == "keyword" {
						es.config.KeywordSuffix = "keyword"
						keywordFound = true
					}
				}
			}
		}

		if keywordFound {
			log.Info("Found Elastic Search keyword suffix to be: %s",
				es.config.KeywordSuffix)
		} else {
			log.Warning("Failed to determine Elastic Search keyword suffix, things may not work right.")
		}
	}

	return nil
}

type DatastoreError struct {
	Message string
	Cause   error
}

func (e *DatastoreError) Error() string {
	if e.Message != "" && e.Cause != nil {
		return fmt.Sprintf("%s: %s", e.Message, e.Cause.Error())
	} else if e.Message != "" {
		return e.Message
	} else if e.Cause != nil {
		return e.Cause.Error()
	}
	return ""
}

func (es *ElasticSearch) Search(query interface{}) (*Response, error) {
	if es.config.KeywordSuffix == "" && !es.config.NoKeywordSuffix {
		log.Warning("Search keyword not known, trying again.")
		es.ConfigureIndex()
	}

	path := fmt.Sprintf("%s/_search?", es.EventSearchIndex)
	if es.MajorVersion == 7 {
		path = fmt.Sprintf("%srest_total_hits_as_int=true&", path)
	}

	if LOG_REQUEST_RESPONSE {
		log.Debug("REQUEST: POST %s: %s", path, util.ToJson(query))
	}

	response, err := es.httpClient.PostJson(path, query)
	if err != nil {
		return nil, errors.WithStack(&DatastoreError{
			Message: "Failed to connect to Elastic Search",
			Cause:   err,
		})
	}

	defer response.Body.Close()
	decodedResponse, err := DecodeResponse(response)

	if LOG_REQUEST_RESPONSE {
		log.Debug("RESPONSE: POST %s: %s", path, decodedResponse.Raw)
	}

	return decodedResponse, err
}

func (es *ElasticSearch) SearchScroll(body interface{}, duration string) (*Response, error) {
	path := fmt.Sprintf("%s/_search?scroll=%s", es.EventSearchIndex, duration)
	response, err := es.httpClient.PostJson(path, body)
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()

	if response.StatusCode != http.StatusOK {
		return nil, DecodeResponseAsError(response)
	}

	return DecodeResponse(response)
}

func (es *ElasticSearch) Scroll(scrollId string, duration string) (*Response, error) {
	body := map[string]interface{}{
		"scroll_id": scrollId,
		"scroll":    duration,
	}
	response, err := es.httpClient.PostJson("_search/scroll", body)
	if err != nil {
		return nil, err
	}
	defer response.Body.Close()

	return DecodeResponse(response)
}

func (es *ElasticSearch) DeleteScroll(scrollId string) (*http.Response, error) {
	return es.httpClient.Delete("_search/scroll", "application/json",
		strings.NewReader(scrollId))
}

func (es *ElasticSearch) PartialUpdate(index string, doctype string, id string,
	doc interface{}) (*http.Response, error) {
	body := map[string]interface{}{
		"doc": doc,
	}
	return es.httpClient.PostJson(fmt.Sprintf("%s/%s/%s/_update?refresh=true",
		index, doctype, id), body)
}

func IsError(response *http.Response) error {
	if response.StatusCode < 400 {
		return nil
	}
	body, err := ioutil.ReadAll(response.Body)
	if err != nil {
		return errors.Wrap(err, "failed to read response")
	}
	return errors.Errorf("%s %s", response.Status, string(body))
}

func (es *ElasticSearch) Update(index string, docType string, docId string,
	body interface{}) (*Response, error) {
	response, err := es.httpClient.PostJson(fmt.Sprintf("%s/%s/%s/_update?refresh=true",
		index, docType, docId), body)
	if err != nil {
		return nil, errors.Wrap(err, "http request failed")
	}
	defer response.Body.Close()
	if err := IsError(response); err != nil {
		return nil, err
	}
	return DecodeResponse(response)
}

// Refresh refreshes all indices logging any error but not returning and
// discarding the response so the caller doesn't have to.
func (es *ElasticSearch) Refresh() {
	response, err := es.httpClient.PostString("_refresh", "application/json", "{}")
	if err != nil {
		log.Error("Failed to refresh Elastic Search: %v", err)
		return
	}
	io.Copy(ioutil.Discard, response.Body)
}

func (es *ElasticSearch) FormatKeyword(keyword string) string {
	if es.config.KeywordSuffix == "" {
		return keyword
	}
	return fmt.Sprintf("%s.%s", keyword, es.config.KeywordSuffix)
}

func (s *ElasticSearch) doUpdateByQuery(query interface{}) (util.JsonMap, error) {
	var response util.JsonMap
	rawResponse, err := s.httpClient.PostJson(
		fmt.Sprintf("%s/_update_by_query?refresh=true&conflicts=proceed",
			s.EventSearchIndex), query)
	if err != nil {
		return nil, errors.Wrap(err, "request failed")
	}
	defer rawResponse.Body.Close()

	body, err := ioutil.ReadAll(rawResponse.Body)
	if err != nil {
		return nil, err
	}

	decoder := json.NewDecoder(bytes.NewReader(body))
	decoder.UseNumber()
	if err := decoder.Decode(&response); err != nil {
		return nil, err
	}

	return response, nil
}

// FormatTimestampUTC formats a time.Time into the format generally used
// by Elastic Search, in particular the @timestamp field.
func FormatTimestampUTC(timestamp time.Time) string {
	return timestamp.UTC().Format("2006-01-02T15:04:05.000Z")
}

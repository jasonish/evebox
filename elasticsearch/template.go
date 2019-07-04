/* Copyright (c) 2016-2018 Jason Ish
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
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/resources"
	"net/http"
)

// TemplateExists checks if a template exists on the server.
func (es *ElasticSearch) TemplateExists(name string) (exists bool, err error) {
	response, err := es.httpClient.Head(fmt.Sprintf("_template/%s", name))
	if err == nil {
		return response.StatusCode == 200, nil
	}
	return false, err
}

// Load an index template into Elasticsearch.
func (es *ElasticSearch) LoadTemplate() error {

	var templateFilename string

	pingResponse, err := es.Ping()
	if err != nil {
		log.Warning("Failed to ping Elastic Search: %v", err)
		return err
	}
	majorVersion := pingResponse.MajorVersion()

	if majorVersion >= 7 {
		templateFilename = "template-es7x.json"
	} else if majorVersion == 6 {
		templateFilename = "template-es6x.json"
	} else if majorVersion == 5 {
		templateFilename = "template-es5x.json"
	} else {
		return fmt.Errorf("No template for Elastic Search with major version %d", majorVersion)
	}

	templateFile, err := resources.GetReader(fmt.Sprintf("elasticsearch/%s", templateFilename))
	if err != nil {
		return err
	}

	decoder := json.NewDecoder(templateFile)
	decoder.UseNumber()

	var template map[string]interface{}
	err = decoder.Decode(&template)
	if err != nil {
		return err
	}
	template["template"] = fmt.Sprintf("%s-*", es.EventIndexPrefix)

	log.Info("Loading template %s for index %s", templateFilename, es.EventIndexPrefix)

	response, err := es.httpClient.PutJson(fmt.Sprintf("_template/%s", es.EventIndexPrefix), template)
	if err != nil {
		return err
	}
	if response.StatusCode != http.StatusOK {
		return DecodeResponseAsError(response)
	}
	es.httpClient.DiscardResponse(response)

	// Now reconfigure the index.
	es.ConfigureIndex()

	// Success.
	return nil
}

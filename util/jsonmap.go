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

package util

import (
	"encoding/json"
	"github.com/jasonish/evebox/log"
)

// A wrapper around a generic string map for accessing elements.
type JsonMap map[string]interface{}

func (m JsonMap) GetMap(name string) JsonMap {
	if m == nil {
		return nil
	}
	val := m[name]
	if val != nil {
		return val.(map[string]interface{})
	}
	return nil
}

func (m JsonMap) GetMapList(name string) []JsonMap {
	if m == nil {
		return nil
	}

	switch v := m[name].(type) {
	case []interface{}:
		result := make([]JsonMap, 0)
		for _, item := range v {
			result = append(result, JsonMap(item.(map[string]interface{})))
		}
		return result
	}

	return nil
}

func (m JsonMap) Get(name string) interface{} {
	if m == nil {
		return nil
	}
	return m[name]
}

func (m JsonMap) GetString(name string) string {
	if m == nil {
		return ""
	}
	if m[name] == nil {
		return ""
	}
	val, ok := m[name].(string)
	if !ok {
		return ""
	}
	return val
}

func (m JsonMap) GetInt64(name string) int64 {
	number, ok := m[name].(json.Number)
	if ok {
		value, err := number.Int64()
		if err == nil {
			return value
		} else {
			fvalue, err := number.Float64()
			if err == nil {
				return int64(fvalue)
			}
		}
	} else {
		log.Warning("Failed to convert %v to json.Number", m[name])
	}
	return 0
}

func (m JsonMap) GetKeys() []string {
	keys := make([]string, 0)
	for key := range (m) {
		keys = append(keys, key)
	}
	return keys
}

func (m JsonMap) HasKey(key string) bool {
	if m[key] == nil {
		return false
	}
	return true
}

// GetAsStrings will return the value with the given name as a slice
// of strings. On failure an empty slice will be returned.
func (m JsonMap) GetAsStrings(name string) []string {
	if m[name] == nil {
		return []string{}
	}
	items, ok := m[name].([]interface{})
	if !ok {
		return []string{}
	}
	strings := make([]string, 0, len(items))
	if items != nil {
		for _, item := range items {
			strings = append(strings, item.(string))
		}

	}
	return strings
}

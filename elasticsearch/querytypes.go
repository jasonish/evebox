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

import "fmt"

func ExistsQuery(field string) interface{} {
	return map[string]interface{}{
		"exists": map[string]interface{}{
			"field": field,
		},
	}
}

func TermQuery(field string, value interface{}) map[string]interface{} {
	return map[string]interface{}{
		"term": map[string]interface{}{
			field: value,
		},
	}
}

func PrefixQuery(field string, value interface{}) map[string]interface{} {
	return map[string]interface{}{
		"prefix": map[string]interface{}{
			field: value,
		},
	}
}

func KeywordTermQuery(field string, value string, suffix string) map[string]interface{} {
	term := field
	if suffix != "" {
		term = fmt.Sprintf("%s.%s", field, suffix)
	}
	return TermQuery(term, value)
}

func KeywordPrefixQuery(field string, value string, suffix string) map[string]interface{} {
	term := field
	if suffix != "" {
		term = fmt.Sprintf("%s.%s", field, suffix)
	}
	return PrefixQuery(term, value)
}

func QueryString(query string) map[string]interface{} {
	return map[string]interface{}{
		"query_string": map[string]interface{}{
			"query":            query,
			"default_operator": "AND",
		},
	}
}

func Sort(field string, order string) map[string]interface{} {
	return map[string]interface{}{
		field: map[string]interface{}{
			"order": order,
		},
	}
}

func Range(rangeType string, field string, value interface{}) interface{} {
	return map[string]interface{}{
		"range": map[string]interface{}{
			field: map[string]interface{}{
				rangeType: value,
			},
		},
	}
}

func RangeGte(field string, value interface{}) interface{} {
	return Range("gte", field, value)
}

func RangeLte(field string, value interface{}) interface{} {
	return Range("lte", field, value)
}

func TopHitsAgg(field string, order string, size int64) interface{} {
	return map[string]interface{}{
		"top_hits": map[string]interface{}{
			"sort": []map[string]interface{}{
				map[string]interface{}{
					field: map[string]interface{}{
						"order": order,

						// Probably need to make this
						// a function parameter.
						"unmapped_type": "long",
					},
				},
			},
			"size": size,
		},
	}
}

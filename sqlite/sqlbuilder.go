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

package sqlite

import (
	"fmt"
	"github.com/jasonish/evebox/log"
)

type SqlBuilder struct {
	prefix string
	from   map[string]bool
	where  []string
	args   []interface{}
}

func (b *SqlBuilder) From(table string) {
	if b.from == nil {
		b.from = make(map[string]bool)
	}
	b.from[table] = true
}

func (b *SqlBuilder) Where(where string) {
	b.where = append(b.where, where)
}

func (b *SqlBuilder) WhereEquals(field string, value interface{}) {
	b.where = append(b.where, fmt.Sprintf("%s = ?", field))
	b.args = append(b.args, value)
}

func (b *SqlBuilder) WhereLte(field string, value interface{}) {
	b.where = append(b.where, fmt.Sprintf("%s <= ?", field))
	b.args = append(b.args, value)
}

func (b *SqlBuilder) WhereGte(field string, value interface{}) {
	b.where = append(b.where, fmt.Sprintf("%s >= ?", field))
	b.args = append(b.args, value)
}

func (b *SqlBuilder) HasWhere() bool {
	return len(b.where) > 0
}

func (b *SqlBuilder) Build() string {
	sql := b.prefix

	sql += b.BuildFrom()

	if b.HasWhere() {
		sql += b.BuildWhere()
	}

	return sql
}

func (b *SqlBuilder) BuildFrom() string {
	sql := " FROM "

	idx := 0
	for table, _ := range b.from {
		log.Println(table)
		if idx > 0 {
			sql += ", "
		}
		sql += table
		idx++
	}

	log.Println(sql)

	return sql
}

func (b *SqlBuilder) BuildWhere() string {
	sql := " WHERE "

	for idx, where := range b.where {
		if idx > 0 {
			sql += " AND "
		}
		sql += where
	}

	return sql
}

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

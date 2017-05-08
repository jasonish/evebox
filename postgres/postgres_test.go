package postgres

import (
	"github.com/stretchr/testify/require"
	"testing"
)

func TestInboxQuery(t *testing.T) {
	r := require.New(t)

	pgConfig, err := ManagedConfig(".")
	r.Nil(err)

	pg, err := NewPgDatabase(pgConfig)
	r.Nil(err)
	r.NotNil(pg)

	pg.Query(``)
}

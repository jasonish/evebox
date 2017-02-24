package sqlite

import (
	"github.com/stretchr/testify/assert"
	"testing"
)

func TestMissingEndQuote(t *testing.T) {
	qs := "\"quoted string missing end"
	p := NewQueryStringParser(qs)
	k, v := p.Next()
	assert.Equal(t, "", k)
	assert.Equal(t, "\"quoted string missing end", v)

	k, v = p.Next()
	assert.Equal(t, "", k)
	assert.Empty(t, "", v)
}

func TestStringWithTrailingWhiteSpace(t *testing.T) {
	qs := "testing "
	p := NewQueryStringParser(qs)

	k, v := p.Next()
	assert.Empty(t, k)
	assert.Equal(t, "testing", v)

	k, v = p.Next()
}

func TestSingleQuotedValue(t *testing.T) {
	qs := "\"quoted string\""
	p := NewQueryStringParser(qs)
	k, v := p.Next()
	assert.Equal(t, "", k)
	assert.Equal(t, "quoted string", v)

	k, v = p.Next()
	assert.Equal(t, "", k)
	assert.Empty(t, "", v)
}

func TestMultipleQuotedValues(t *testing.T) {
	qs := "\"quoted string\" \"and another one\""
	p := NewQueryStringParser(qs)
	k, v := p.Next()
	assert.Empty(t, k)
	assert.Equal(t, "quoted string", v)

	k, v = p.Next()
	assert.Empty(t, k)
	assert.Equal(t, "and another one", v)
}

func TestSingleValue(t *testing.T) {
	qs := "justonelongstringperhapswithsome\"*&specialchars"
	p := NewQueryStringParser(qs)

	k, v := p.Next()
	assert.Empty(t, k)
	assert.Equal(t, qs, v)
}

func TestMultipleUnquotedValues(t *testing.T) {
	qs := "one two three"
	p := NewQueryStringParser(qs)

	k, v := p.Next()
	assert.Empty(t, k)
	assert.Equal(t, "one", v)

	k, v = p.Next()
	assert.Empty(t, k)
	assert.Equal(t, "two", v)

	k, v = p.Next()
	assert.Empty(t, k)
	assert.Equal(t, "three", v)
}

func TestSingleKeyVal(t *testing.T) {
	qs := "key:val"
	p := NewQueryStringParser(qs)

	k, v := p.Next()
	assert.Equal(t, "key", k)
	assert.Equal(t, "val", v)
}

func TestMultipleKeyVals(t *testing.T) {
	qs := "key1:val1 key2:val2"
	p := NewQueryStringParser(qs)

	k, v := p.Next()
	assert.Equal(t, "key1", k)
	assert.Equal(t, "val1", v)

	k, v = p.Next()
	assert.Equal(t, "key2", k)
	assert.Equal(t, "val2", v)
}

func TestQuotedVal(t *testing.T) {
	qs := "key1:val1 key2:val2 key3:\"this is key 3\""
	p := NewQueryStringParser(qs)

	k, v := p.Next()
	assert.Equal(t, "key1", k)
	assert.Equal(t, "val1", v)

	k, v = p.Next()
	assert.Equal(t, "key2", k)
	assert.Equal(t, "val2", v)

	k, v = p.Next()
	assert.Equal(t, "key3", k)
	assert.Equal(t, "this is key 3", v)
}

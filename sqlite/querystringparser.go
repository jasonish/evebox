package sqlite

import (
	"strings"
)

type QueryStringParser struct {
	q string
}

func NewQueryStringParser(q string) *QueryStringParser {
	return &QueryStringParser{
		q: q,
	}
}

func (p *QueryStringParser) nextString() (s string) {
	// Skip leading white space.
	for {
		if p.q[0] == ' ' {
			p.q = p.q[1:]
		} else {
			break
		}
	}

	if p.q[0] == '"' {
		end := strings.IndexByte(p.q[1:], '"')
		if end < 0 {
			s = p.q[0:]
			p.q = ""
		} else {
			s = p.q[1 : end+1]
			p.q = p.q[end+2:]
		}
	} else {
		end := strings.IndexByte(p.q, ' ')
		if end < 0 {
			s = p.q[0:]
			p.q = ""
		} else {
			s = p.q[0:end]
			p.q = p.q[end:]
		}
	}

	return s
}

func (p *QueryStringParser) Next() (string, string) {
	// Skip leading white space.
	for {
		if len(p.q) == 0 {
			return "", ""
		}
		if p.q[0] == ' ' {
			p.q = p.q[1:]
		} else {
			break
		}
	}

	if len(p.q) == 0 {
		return "", ""
	}

	// Quoted string right at the start, its not a key/value.
	if p.q[0] == '"' {
		return "", p.nextString()
	}

	// Find the next separator.
	sep := strings.IndexAny(p.q, " :")

	// If no separator found, return the remainder as one value.
	if sep < 0 {
		next := p.q[0:]
		p.q = ""
		return "", next
	}

	// If separator is a space, return the next string as a value.
	if p.q[sep] == ' ' {
		return "", p.nextString()
	}

	// Looks like a key value pair.
	key := p.q[0:sep]
	p.q = p.q[sep+1:]
	val := p.nextString()

	return key, val
}

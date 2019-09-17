// The MIT License (MIT)
// Copyright (c) 2016 Jason Ish
//
// Permission is hereby granted, free of charge, to any person
// obtaining a copy of this software and associated documentation
// files (the "Software"), to deal in the Software without
// restriction, including without limitation the rights to use, copy,
// modify, merge, publish, distribute, sublicense, and/or sell copies
// of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be
// included in all copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND
// NONINFRINGEMENT. IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS
// BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN
// ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR IN
// CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.

package ruleparser

import (
	"bufio"
	"fmt"
	"io"
	"strconv"
	"strings"
)

func newIncompleteRuleError() error {
	return fmt.Errorf("incomplete")
}

// Remove leading and trailing quotes from a string.
func trimQuotes(buf string) string {
	buflen := len(buf)
	if buflen == 0 {
		return buf
	}
	if buf[0:1] == "\"" && buf[buflen-1:buflen] == "\"" {
		return buf[1: buflen-1]
	}
	return buf
}

// Remove leading white space from a string.
func trimLeadingWhiteSpace(buf string) string {
	return strings.TrimLeft(buf, " ")
}

func splitAt(buf string, sep string) (string, string) {
	var leading string
	var trailing string

	parts := strings.SplitN(buf, sep, 2)
	if len(parts) > 1 {
		trailing = strings.TrimSpace(parts[1])
	}
	leading = strings.TrimSpace(parts[0])

	return leading, trailing
}

// Parse the next rule option from the provided rule.
//
// The option, argument and the remainder of the rule are returned.
func parseOption(rule string) (string, string, string, error) {
	var option string
	var arg string

	// Strip any leading space.
	rule = trimLeadingWhiteSpace(rule)

	hasArg := false
	optend := strings.IndexFunc(rule, func(r rune) bool {
		switch r {
		case ';':
			return true
		case ':':
			hasArg = true
			return true
		}
		return false
	})
	if optend < 0 {
		return option, arg, rule, fmt.Errorf("unterminated option")
	}

	option = rule[0:optend]

	rule = rule[optend+1:]

	if hasArg {
		if len(rule) == 0 {
			return option, arg, rule, fmt.Errorf("no argument")
		}
		escaped := false
		argend := strings.IndexFunc(rule, func(r rune) bool {
			if escaped {
				escaped = false
			} else if r == '\\' {
				escaped = true
			} else if r == ';' {
				return true
			}
			return false
		})
		if argend < 0 {
			return option, arg, rule,
				fmt.Errorf("unterminated option argument")
		}
		arg = rule[:argend]
		rule = rule[argend+1:]
	}

	return option, trimQuotes(arg), rule, nil
}

// Parse an IDS rule from the provided string buffer.
func Parse(buf string) (Rule, error) {
	rule := Rule{
		Raw: buf,
	}

	// Removing leading space.
	buf = trimLeadingWhiteSpace(buf)

	// Check enable/disable status.
	if !strings.HasPrefix(buf, "#") {
		rule.Enabled = true
	} else {
		buf = strings.TrimPrefix(buf, "#")
		buf = trimLeadingWhiteSpace(buf)
	}

	action, rem := splitAt(buf, " ")
	rule.Action = action
	if len(rem) == 0 {
		return rule, newIncompleteRuleError()
	}

	proto, rem := splitAt(rem, " ")
	rule.Proto = proto
	if len(rem) == 0 {
		return rule, newIncompleteRuleError()
	}

	sourceAddr, rem := splitAt(rem, " ")
	rule.SourceAddr = sourceAddr
	if len(rem) == 0 {
		return rule, newIncompleteRuleError()
	}

	sourcePort, rem := splitAt(rem, " ")
	rule.SourcePort = sourcePort
	if len(rem) == 0 {
		return rule, newIncompleteRuleError()
	}

	direction, rem := splitAt(rem, " ")
	if !validateDirection(direction) {
		return rule, fmt.Errorf("invalid direction: %s", direction)
	}
	rule.Direction = direction
	if len(rem) == 0 {
		return rule, newIncompleteRuleError()
	}

	destAddr, rem := splitAt(rem, " ")
	rule.DestAddr = destAddr
	if len(rem) == 0 {
		return rule, newIncompleteRuleError()
	}

	destPort, rem := splitAt(rem, " ")
	rule.DestPort = destPort
	if len(rem) == 0 {
		return rule, newIncompleteRuleError()
	}

	offset := 0

	// Check that then next char is a (.
	if rem[offset:offset+1] != "(" {
		return rule, fmt.Errorf("expected (, got %s", rem[0:1])
	}
	offset++

	buf = rem[offset:]

	// Parse options.
	var option string
	var arg string
	var err error
	for {
		if len(buf) == 0 {
			return rule, newIncompleteRuleError()
		}

		buf = trimLeadingWhiteSpace(buf)

		if strings.HasPrefix(buf, ")") {
			// Done.
			break
		}

		option, arg, buf, err = parseOption(buf)
		if err != nil {
			return rule, err
		}

		ruleOption := RuleOption{option, arg}
		rule.Options = append(rule.Options, ruleOption)

		switch option {
		case "msg":
			rule.Msg = arg
		case "sid":
			sid, err := strconv.ParseUint(arg, 10, 64)
			if err != nil {
				return rule, fmt.Errorf("failed to parse sid: %s", arg)
			}
			rule.Sid = sid
		case "gid":
			gid, err := strconv.ParseUint(arg, 10, 64)
			if err != nil {
				return rule, fmt.Errorf("failed to parse sid: %s", arg)
			}
			rule.Gid = gid
		}
	}

	return rule, nil
}

// ParseReader parses multiple rules from a reader.
func ParseReader(reader io.Reader) ([]Rule, error) {
	rules := make([]Rule, 0)

	ruleReader := NewRuleReader(reader)

	for {
		rule, err := ruleReader.Next()
		if err != nil {
			if err == io.EOF {
				break
			}
			continue
		}
		rules = append(rules, rule)
	}

	return rules, nil
}

// RuleReader parses rules one by from an underlying reader.
type RuleReader struct {
	reader *bufio.Reader
}

// NewRuleReader creates a new RuleReader reading from a reader.
func NewRuleReader(reader io.Reader) *RuleReader {
	ruleReader := &RuleReader{
		reader: bufio.NewReader(reader),
	}
	return ruleReader
}

func (r *RuleReader) readLine() (string, error) {
	bytes, err := r.reader.ReadBytes('\n')
	if err != nil && len(bytes) == 0 {
		return "", err
	}
	return strings.TrimSpace(string(bytes)), nil
}

// Next returns the next rule read from the reader. Empty lines and commented
// out lines are skipped. Any other line that doesn't parse as a rule is
// considered an error.
func (r *RuleReader) Next() (Rule, error) {

	ruleString := ""

	for {
		line, err := r.readLine()
		if err != nil && line == "" {
			return Rule{}, err
		}

		if len(line) == 0 {
			continue
		}

		if strings.HasSuffix(line, "\\") {
			ruleString = fmt.Sprintf("%s%s",
				ruleString, line[0:len(line)-1])
			continue
		}

		ruleString = fmt.Sprintf("%s%s", ruleString, line)

		rule, err := Parse(ruleString)
		if err != nil {
			if strings.HasPrefix(ruleString, "#") {
				ruleString = ""
				continue
			}
			return Rule{}, err
		}
		ruleString = ""
		return rule, err
	}

}

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

// RuleOption is a struct representing an IDS rule option.
type RuleOption struct {
	Option string `json:"option"`
	Args   string `json:"args"`
}

// Rule is a struct representing an IDS rule.
type Rule struct {
	// The raw rule string.
	Raw string

	Enabled bool

	// Header components.
	Action     string
	Proto      string
	SourceAddr string
	SourcePort string
	Direction  string
	DestAddr   string
	DestPort   string

	// List of options in order.
	Options []RuleOption

	// Some options are also pulled out for easy access.
	Msg string
	Sid uint64
	Gid uint64
}

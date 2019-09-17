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
	"os"
	"strings"
	"testing"

	"github.com/stretchr/testify/assert"
	"io"
)

func TestSplitAt(t *testing.T) {
	leading, trailing := splitAt("one", " ")
	if leading != "one" {
		t.Fatal("not equal to 'one'")
	}
	if trailing != "" {
		t.Fatal("expected empty string")
	}
}

var validRuleTests = []struct {
	input  string
	output Rule
}{
	{
		// From ET Open, Suricata 3.1.
		`alert tcp $EXTERNAL_NET $HTTP_PORTS -> $HOME_NET any (msg:"ET ACTIVEX Possible NOS Microsystems Adobe Reader/Acrobat getPlus Get_atlcomHelper ActiveX Control Multiple Stack Overflows Remote Code Execution Attempt"; flow:established,to_client; content:"E2883E8F-472F-4fb0-9522-AC9BF37916A7"; nocase; content:"offer-"; nocase; pcre:"/<OBJECT\s+[^>]*classid\s*=\s*[\x22\x27]?\s*clsid\s*\x3a\s*\x7B?\s*E2883E8F-472F-4fb0-9522-AC9BF37916A7.+offer-(ineligible|preinstalled|declined|accepted)/si"; reference:url,www.securityfocus.com/bid/37759; reference:url,www.kb.cert.org/vuls/id/773545; reference:url,www.adobe.com/support/security/bulletins/apsb10-02.html; reference:url,www.exploit-db.com/exploits/11172/; reference:cve,2009-3958; reference:url,doc.emergingthreats.net/2010665; classtype:attempted-user; sid:2010665; rev:7;)`,
		Rule{
			Action:     "alert",
			Proto:      "tcp",
			SourceAddr: "$EXTERNAL_NET",
			SourcePort: "$HTTP_PORTS",
			Direction:  "->",
			DestAddr:   "$HOME_NET",
			DestPort:   "any",
			Msg:        `ET ACTIVEX Possible NOS Microsystems Adobe Reader/Acrobat getPlus Get_atlcomHelper ActiveX Control Multiple Stack Overflows Remote Code Execution Attempt`,
			Sid:        uint64(2010665),
			Gid:        uint64(0),
		},
	},

	{
		// From ET Open, Suricata 3.1 (ciarmy.rules).
		`alert ip [1.34.6.220,1.34.12.196,1.34.12.225,1.34.15.234,1.34.35.168,1.34.36.11,1.34.36.80,1.34.40.246,1.34.54.20,1.34.70.86,1.34.85.46,1.34.93.100,1.34.118.108,1.34.130.153,1.34.139.167,1.34.158.144,1.34.165.112,1.34.168.202,1.34.197.19,1.34.198.134,1.34.200.111,1.34.208.161,1.34.221.165,1.34.243.195,1.34.244.43,1.34.250.244,1.52.54.254,1.52.93.249,1.53.64.47,1.53.143.147,1.53.202.61,1.58.173.68,1.62.120.17,1.62.252.210,1.65.165.91,1.162.169.124,1.162.173.232,1.162.233.25,1.162.235.8,1.179.153.114,1.180.237.106,1.180.237.107,1.180.237.108,1.180.237.109,1.182.249.151,1.186.60.148,1.186.234.88,1.192.144.183,1.217.127.106,1.230.45.179] any -> $HOME_NET any (msg:"ET CINS Active Threat Intelligence Poor Reputation IP group 1"; reference:url,www.cinsscore.com; reference:url,www.networkcloaking.com/cins; threshold: type limit, track by_src, seconds 3600, count 1; classtype:misc-attack; sid:2403300; rev:3064;)`,
		Rule{
			Action:     "alert",
			Proto:      "ip",
			SourceAddr: "[1.34.6.220,1.34.12.196,1.34.12.225,1.34.15.234,1.34.35.168,1.34.36.11,1.34.36.80,1.34.40.246,1.34.54.20,1.34.70.86,1.34.85.46,1.34.93.100,1.34.118.108,1.34.130.153,1.34.139.167,1.34.158.144,1.34.165.112,1.34.168.202,1.34.197.19,1.34.198.134,1.34.200.111,1.34.208.161,1.34.221.165,1.34.243.195,1.34.244.43,1.34.250.244,1.52.54.254,1.52.93.249,1.53.64.47,1.53.143.147,1.53.202.61,1.58.173.68,1.62.120.17,1.62.252.210,1.65.165.91,1.162.169.124,1.162.173.232,1.162.233.25,1.162.235.8,1.179.153.114,1.180.237.106,1.180.237.107,1.180.237.108,1.180.237.109,1.182.249.151,1.186.60.148,1.186.234.88,1.192.144.183,1.217.127.106,1.230.45.179]",
			SourcePort: "any",
			Direction:  "->",
			DestAddr:   "$HOME_NET",
			DestPort:   "any",
			Msg:        "ET CINS Active Threat Intelligence Poor Reputation IP group 1",
			Sid:        uint64(2403300),
			Gid:        uint64(0),
		},
	},
}

func TestValidRules(t *testing.T) {
	for _, test := range validRuleTests {

		rule, err := Parse(test.input)
		if err != nil {
			t.Errorf("Failed to parse: %s", test.input)
		}
		if rule.Action != test.output.Action {
			t.Errorf("invalid action, expected |%s|, got |%s|",
				test.output.Action, rule.Action)
		}
		assert.Equal(t, test.input, rule.Raw)
		assert.Equal(t, test.output.SourceAddr, rule.SourceAddr)
		assert.Equal(t, test.output.SourcePort, rule.SourcePort)
		assert.Equal(t, test.output.DestAddr, rule.DestAddr)
		assert.Equal(t, test.output.DestPort, rule.DestPort)
		assert.Equal(t, test.output.Proto, rule.Proto)
		assert.Equal(t, test.output.Msg, rule.Msg)
		assert.Equal(t, test.output.Sid, rule.Sid)
		assert.Equal(t, test.output.Gid, rule.Gid)
	}
}

// Test parsing various levels of an incomplete rule.

var incompleteTests = []struct {
	input string
	err   bool
}{
	{"alert", true},
	{"alert ", true},
	{"alert tcp", true},
	{"alert tcp ", true},
	{"alert tcp any", true},
	{"alert tcp any ", true},
	{"alert tcp any any", true},
	{"alert tcp any any ", true},
	{"alert tcp any any ->", true},
	{"alert tcp any any -> ", true},
	{"alert tcp any any -> any", true},
	{"alert tcp any any -> any ", true},
	{"alert tcp any any -> any any", true},
	{"alert tcp any any -> any any ", true},
	{"alert tcp any any -> any any (", true},
	{`alert tcp any any -> any any (msg`, true},
	{`alert tcp any any -> any any (msg:`, true},
	{`alert tcp any any -> any any (msg:"some message`, true},
	{`alert tcp any any -> any any (msg:"some message"`, true},
	{`alert tcp any any -> any any (msg:"some message";`, true},
	{`alert tcp any any -> any any (msg:"some message"; sid`, true},
	{`alert tcp any any -> any any (msg:"some message"; sid:`, true},
	{`alert tcp any any -> any any (msg:"some message"; sid:1`, true},
	{`alert tcp any any -> any any (msg:"some message"; sid:1;)`, false},
}

func TestIncompleteRules(t *testing.T) {
	for _, test := range incompleteTests {
		_, err := Parse(test.input)
		if test.err {
			assert.NotNil(t, err)
		} else {
			assert.Nil(t, err)
		}
	}
}

var invalidSidTests = []string{
	`alert tcp any any -> any any (msg:"msg"; sid:-1;)`,
	`alert tcp any any -> any any (msg:"msg"; sid:a;)`,
	`alert tcp any any -> any any (msg:"msg"; sid:18,446,744,073,709,551,615;)`,

	// One over the max value for uint64.
	`alert tcp any any -> any any (msg:"msg"; sid:18446744073709551616;)`,
}

func TestInvalidSids(t *testing.T) {
	for _, test := range invalidSidTests {
		_, err := Parse(test)
		assert.NotNil(t, err, "error expected for rule %s", test)
	}
}

func TestParseOption(t *testing.T) {

	option, arg, rule, err := parseOption(`msg:"Test message"; the rest of the rule...`)
	assert.Nil(t, err)
	assert.Equal(t, "msg", option)
	assert.Equal(t, "Test message", arg)
	assert.Equal(t, " the rest of the rule...", rule)

	_, _, _, err = parseOption(`msg`)
	assert.NotNil(t, err, "error expected")

	_, _, _, err = parseOption(`msg:`)
	assert.NotNil(t, err, "error expected")

	_, _, _, err = parseOption(`msg:"This is a test`)
	assert.NotNil(t, err, "error expected")

	option, arg, rule, err = parseOption("nocase;")
	assert.Nil(t, err, "err should be nil")
	assert.Equal(t, "nocase", option)
	assert.Equal(t, "", arg, "arg")
	assert.Equal(t, "", rule, "remainder of rule")

}

func TestParseFile(t *testing.T) {
	input, err := os.Open("testdata/emerging-telnet.rules")
	assert.Nil(t, err)

	rules, err := ParseReader(input)
	assert.Nil(t, err)
	assert.Equal(t, 12, len(rules))
	assert.Equal(t, "alert", rules[0].Action)
}

func TestParseRuleWithList(t *testing.T) {
	// Address lists should parse.
	buf := `alert tcp [1.1.1.1/32,2.2.2.2/32] any -> any any (msg:"Message"; sid:1; rev:1;)`
	rule, err := Parse(buf)
	assert.Nil(t, err)
	assert.Equal(t, "[1.1.1.1/32,2.2.2.2/32]", rule.SourceAddr)

	// But like Snort, should not parse an address list with spaces.
	buf = `alert tcp [1.1.1.1/32, 2.2.2.2/32] any -> any any (msg:"Message"; sid:1; rev:1;)`
	rule, err = Parse(buf)
	assert.NotNil(t, err)
}

func TestParseMultilineRule(t *testing.T) {
	buf := `alert tcp any any -> any any ( \
msg:\"A multiline rule\"; sid:1;)

alert \
	tcp any any -> any any \
( \
	msg:"A rule split over many lines"; \
sid:2; rev:3; \
)
`
	reader := strings.NewReader(buf)
	rules, err := ParseReader(reader)
	assert.Nil(t, err)
	assert.Equal(t, 2, len(rules))
}

func TestParseEnabledAndDisabled(t *testing.T) {
	// From ET Open, Suricata 3.1 (ciarmy.rules).
	buf := `alert ip [1.34.6.220,1.34.12.196,1.34.12.225,1.34.15.234,1.34.35.168,1.34.36.11,1.34.36.80,1.34.40.246,1.34.54.20,1.34.70.86,1.34.85.46,1.34.93.100,1.34.118.108,1.34.130.153,1.34.139.167,1.34.158.144,1.34.165.112,1.34.168.202,1.34.197.19,1.34.198.134,1.34.200.111,1.34.208.161,1.34.221.165,1.34.243.195,1.34.244.43,1.34.250.244,1.52.54.254,1.52.93.249,1.53.64.47,1.53.143.147,1.53.202.61,1.58.173.68,1.62.120.17,1.62.252.210,1.65.165.91,1.162.169.124,1.162.173.232,1.162.233.25,1.162.235.8,1.179.153.114,1.180.237.106,1.180.237.107,1.180.237.108,1.180.237.109,1.182.249.151,1.186.60.148,1.186.234.88,1.192.144.183,1.217.127.106,1.230.45.179] any -> $HOME_NET any (msg:"ET CINS Active Threat Intelligence Poor Reputation IP group 1"; reference:url,www.cinsscore.com; reference:url,www.networkcloaking.com/cins; threshold: type limit, track by_src, seconds 3600, count 1; classtype:misc-attack; sid:2403300; rev:3064;)`

	rule, err := Parse(buf)
	assert.Nil(t, err)
	assert.True(t, rule.Enabled)

	rule, err = Parse("#" + buf)
	assert.Nil(t, err)
	assert.False(t, rule.Enabled)
}

func TestRuleReader_CommentsAndBlanks(t *testing.T) {
	buf := `# Some comments

# and some blank lines.`
	reader := NewRuleReader(strings.NewReader(buf))
	rule, err := reader.Next()
	// Should get an empty rule.
	assert.Equal(t, Rule{}, rule)

	// And the only error should be EOF.
	assert.Equal(t, io.EOF, err)
}

func TestRuleReader_Multiline(t *testing.T) {
	buf := `alert tcp $EXTERNAL_NET $HTTP_PORTS \
-> $HOME_NET any (msg:"ET \
ACTIVEX Possible NOS Microsystems Adobe Reader/Acrobat getPlus Get_atlcomHelper ActiveX Control Multiple Stack Overflows Remote Code Execution Attempt"; flow:established,to_client; content:"E2883E8F-472F-4fb0-9522-AC9BF37916A7"; nocase; content:"offer-"; nocase; pcre:"/<OBJECT\s+[^>]*classid\s*=\s*[\x22\x27]?\s*clsid\s*\x3a\s*\x7B?\s*E2883E8F-472F-4fb0-9522-AC9BF37916A7.+offer-(ineligible|preinstalled|declined|accepted)/si"; reference:url,www.securityfocus.com/bid/37759; reference:url,www.kb.cert.org/vuls/id/773545; reference:url,www.adobe.com/support/security/bulletins/apsb10-02.html; reference:url,www.exploit-db.com/exploits/11172/; reference:cve,2009-3958; reference:url,doc.emergingthreats.net/2010665; classtype:attempted-user; sid:2010665; rev:7;)`
	reader := NewRuleReader(strings.NewReader(buf))
	rule, _ := reader.Next()
	assert.Equal(t, "ET ACTIVEX Possible NOS Microsystems Adobe Reader/Acrobat getPlus Get_atlcomHelper ActiveX Control Multiple Stack Overflows Remote Code Execution Attempt", rule.Msg)
}

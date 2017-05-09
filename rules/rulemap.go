/* Copyright (c) 2017 Jason Ish
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

package rules

import (
	"github.com/jasonish/evebox/log"
	"path/filepath"
	"os"
	"github.com/jasonish/go-idsrules"
	"io"
	"io/ioutil"
	"strings"
	"github.com/jasonish/evebox/eve"
)

func loadRulesFromFile(ruleMap *map[uint64]idsrules.Rule, filename string) error {
	file, err := os.Open(filename)
	if err != nil {
		return err
	}
	defer file.Close()

	ruleReader := idsrules.NewRuleReader(file)

	count := 0

	for {
		rule, err := ruleReader.Next()
		if err != nil {
			if err == io.EOF {
				break
			}
			if parseError, ok := err.(*idsrules.RuleParseError); ok {
				log.Warning("Rule parse error: %v", parseError)
				continue
			} else {
				log.Error("%v", err)
				break
			}
		}

		if _, ok := (*ruleMap)[rule.Sid]; ok {
			log.Warning("A rule with ID %d already exists.", rule.Sid)
		} else {
			count++
			(*ruleMap)[rule.Sid] = rule
		}

	}

	log.Debug("Loaded %d rules from %s", count, filename)

	return nil
}

type RuleMap struct {
	rules map[uint64]idsrules.Rule
}

func NewRuleMap(paths []string) *RuleMap {

	rules := make(map[uint64]idsrules.Rule)

	for _, path := range (paths) {

		fileInfo, err := os.Stat(path)
		if err != nil {
			// Load as glob.
			matches, err := filepath.Glob(path)
			if err != nil {
				log.Warning("No matches for %s: %v", path, err)
				continue
			}
			for _, m := range (matches) {
				loadRulesFromFile(&rules, m)
			}
		} else if fileInfo.IsDir() {
			infos, err := ioutil.ReadDir(path)
			if err != nil {
				log.Warning("Failed to read %s: %v", fileInfo.Name(), err)
				continue
			}
			for _, info := range infos {
				if !strings.HasSuffix(info.Name(), ".rules") {
					continue
				}
				fullFilename := filepath.Join(path, info.Name())
				log.Info("%s", fullFilename)
				if err := loadRulesFromFile(&rules, fullFilename); err != nil {
					log.Warning("Failed to load %s: %v", fullFilename, err)
				}
			}
		} else {
			loadRulesFromFile(&rules, path)
		}

	}

	log.Info("Loaded %d rules", len(rules))

	return &RuleMap{
		rules: rules,
	}
}

func (r *RuleMap) FindById(id uint64) *idsrules.Rule {
	if r == nil || r.rules == nil {
		return nil
	}
	if rule, ok := r.rules[id]; ok {
		return &rule
	}
	return nil
}

// Filter implements eve.EveFilter for RuleMap.
func (r *RuleMap) Filter(event eve.EveEvent) {
	ruleId, ok := event.GetAlertSignatureId()
	if ok {
		rule := r.FindById(ruleId)
		if rule != nil {
			event["rule"] = rule.Raw
		}
	}
}

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
	"github.com/fsnotify/fsnotify"
	"github.com/jasonish/evebox/eve"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/ruleparser"
	"io"
	"io/ioutil"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"
)

type RuleMap struct {
	paths   []string
	rules   map[uint64]ruleparser.Rule
	watcher *fsnotify.Watcher
	lock    sync.RWMutex
}

func NewRuleMap(paths []string) *RuleMap {

	watcher, err := fsnotify.NewWatcher()
	if err != nil {
		log.Warning("Failed to initialize file watcher, rules won't be automatically reloaded: %v", err)
		watcher = nil
	}

	rulemap := &RuleMap{
		paths:   paths,
		watcher: watcher,
	}

	rulemap.reload()

	go rulemap.watchFiles()

	return rulemap
}

func (r *RuleMap) watchFiles() {

	doReload := false
	lastMod := time.Now()

	timer := time.Tick(1 * time.Second)

	for {
		select {
		case <-r.watcher.Events:
			doReload = true
			lastMod = time.Now()
		case err := <-r.watcher.Errors:
			log.Warning("File watch error: %v", err)
		case <-timer:
			if doReload && time.Now().Sub(lastMod).Seconds() > 1 {
				log.Info("Reloading rules...")
				r.reload()
				doReload = false
			}
		}
	}
}

func (r *RuleMap) reload() {
	rules := make(map[uint64]ruleparser.Rule)
	filenames := findRuleFilenames(r.paths)
	for _, filename := range filenames {
		if err := loadRulesFromFile(&rules, filename); err != nil {
			log.Warning("Failed to load rules from %s: %v", filename, err)
		}
		if err := r.watcher.Add(filename); err != nil {
			log.Warning("Failed to add watch for %s: %v", filename, err)
		}
	}
	r.lock.Lock()
	r.rules = rules
	r.lock.Unlock()
	log.Info("Loaded %d rules", len(rules))
}

func (r *RuleMap) FindById(id uint64) *ruleparser.Rule {
	r.lock.RLock()
	defer r.lock.RUnlock()
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
			alert := event.GetAlert()
			if alert != nil {
				alert["rule"] = rule.Raw
			}
		}
	}
}

func loadRulesFromFile(ruleMap *map[uint64]ruleparser.Rule, filename string) error {
	file, err := os.Open(filename)
	if err != nil {
		return err
	}
	defer file.Close()

	ruleReader := ruleparser.NewRuleReader(file)

	count := 0

	for {
		rule, err := ruleReader.Next()
		if err != nil {
			if err == io.EOF {
				break
			}
			log.Warning("Rule parser error: %v", err)
			continue
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

// findRuleFilenames returns a list of full file names from the
// paths/patterns provided.
func findRuleFilenames(paths []string) []string {
	filenames := make([]string, 0)

	for _, path := range paths {
		fileInfo, err := os.Stat(path)
		if err != nil {
			// Load as glob.
			matches, err := filepath.Glob(path)
			if err != nil {
				log.Warning("Failed to load glob %s: %v", path, err)
			} else if len(matches) == 0 {
				log.Warning("No files matched glob %s.", path)
			} else {
				for _, m := range matches {
					filenames = append(filenames, m)
				}
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
				filenames = append(filenames, fullFilename)
			}
		} else {
			filenames = append(filenames, path)
		}
	}

	return filenames
}

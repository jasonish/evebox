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

package postgres

import (
	"bufio"
	"fmt"
	"github.com/jasonish/evebox/log"
	"github.com/pkg/errors"
	"io"
	"io/ioutil"
	"os"
	"os/exec"
	"path"
	"path/filepath"
	"strings"
	"syscall"
)

func GetVersion() (*PostgresVersion, error) {
	command := exec.Command("postgres", "--version")
	stdout, err := command.StdoutPipe()
	if err != nil {
		return nil, err
	}
	err = command.Start()
	if err != nil {
		return nil, err
	}
	output, err := ioutil.ReadAll(stdout)
	if err != nil {
		return nil, err
	}
	versionString := string(output)
	command.Wait()

	return ParseVersion(versionString)
}

type PostgresManager struct {
	directory string
	command   *exec.Cmd
	running   bool
	onReady   chan bool
}

func NewPostgresManager(directory string) (*PostgresManager, error) {
	version, err := GetVersion()
	if err != nil {
		return nil, err
	}

	absDataDirectory, err := filepath.Abs(directory)
	if err != nil {
		return nil, err
	}

	path := path.Join(absDataDirectory,
		fmt.Sprintf("pgdata%s", version.MajorMinor))
	return &PostgresManager{
		directory: path,
	}, nil
}

func (p *PostgresManager) pipeReader(pipe io.ReadCloser, logPrefix string) error {
	reader := bufio.NewReader(pipe)
	for {
		line, err := reader.ReadBytes('\n')
		if err != nil && err == io.EOF {
			break
		} else if err != nil {
			return err
		}
		if !p.running {
			if strings.Index(string(line), "database system is ready") > -1 {
				p.running = true
				p.onReady <- true
			}
		}
		log.Info("%s: %s", logPrefix, strings.TrimSpace(string(line)))
	}
	return nil
}

func (p *PostgresManager) IsInitialized() bool {
	_, err := os.Stat(p.directory)
	if err == nil {
		return true
	}
	return false
}

func (p *PostgresManager) Init() error {
	command := exec.Command("initdb",
		"-D", p.directory,
		fmt.Sprintf("--username=%s", PGUSER),
		"--encoding=UTF8")

	stdout, err := command.StdoutPipe()
	if err != nil {
		log.Error("Failed to open initdb stdout, will not be logged.")
		stdout = nil
	}

	stderr, err := command.StderrPipe()
	if err != nil {
		log.Error("Failed to open initdb stderr, will not be logged.")
		stderr = nil
	}

	err = command.Start()
	if err != nil {
		log.Error("Failed to start initdb: %v", err)
		return err
	}

	go p.pipeReader(stdout, "initdb stdout")
	go p.pipeReader(stderr, "initdb stderr")

	if err := command.Wait(); err != nil {
		return err
	}

	if err := p.Start(); err != nil {
		return errors.Wrap(err, "failed to start")
	}
	defer p.StopFast()

	pgConfig := PgConfig{
		User:     PGUSER,
		Password: PGPASS,
		Database: "postgres",
		Host:     p.directory,
	}
	db, err := NewPgDatabase(pgConfig)
	if err != nil {
		return nil
	}
	defer db.Close()

	_, err = db.Exec(fmt.Sprintf("create database %s", PGDATABASE))
	if err != nil {
		return errors.Wrap(err, "failed to execute create database command")
	}

	return nil
}

func (p *PostgresManager) Start() error {

	if p.running {
		return errors.New("already running")
	}

	// Get the absolute path of the data directory.
	path, err := filepath.Abs(p.directory)
	if err != nil {
		return err
	}
	log.Info("Using postgres data directory %s", path)

	p.command = exec.Command("postgres",
		"-D", path,
		"-c", "log_destination=stderr",
		"-c", "logging_collector=off",
		"-c", "listen_addresses=127.0.0.1",
		"-k", path)

	stdout, err := p.command.StdoutPipe()
	if err != nil {
		log.Error("Failed to open postgres stdout, will not be logged.")
		stdout = nil
	}

	stderr, err := p.command.StderrPipe()
	if err != nil {
		log.Error("Failed to open postgres stderr, will not be logged.")
		stderr = nil
	}

	err = p.command.Start()
	if err != nil {
		log.Error("Failed to start postgres: %v", err)
		return err
	}

	p.onReady = make(chan bool)

	go p.pipeReader(stdout, "postgres stdout")
	go p.pipeReader(stderr, "postgres stderr")

	log.Info("Waiting for PostgreSQL to be running...")
	<-p.onReady

	return nil
}

func (p *PostgresManager) stop(sig syscall.Signal) {
	if p.command == nil {
		return
	}
	p.command.Process.Signal(sig)
	p.command.Wait()
	p.running = false
}

func (p *PostgresManager) StopSmart() {
	p.stop(syscall.SIGTERM)
}

func (p *PostgresManager) StopFast() {
	p.stop(syscall.SIGINT)
}

func (p *PostgresManager) StopImmediate() {
	p.stop(syscall.SIGQUIT)
}

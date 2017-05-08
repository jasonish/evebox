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
	"database/sql"
	"fmt"
	"github.com/jasonish/evebox/log"
	_ "github.com/lib/pq"
	"github.com/pkg/errors"
	"path"
	"path/filepath"
)

const driver = "postgres"

const PGDATABASE = "evebox"
const PGUSER = "evebox"
const PGPASS = "evebox"

type PgConfig struct {
	Database string
	User     string
	Password string
	Host     string
}

type Service struct {
	db *sql.DB
}

type PgDB struct {
	*sql.DB
}

func NewPgDatabase(config PgConfig) (*PgDB, error) {
	dsn := fmt.Sprintf("dbname=%s user=%s password=%s host=%s",
		config.Database, config.User, config.Password, config.Host)
	db, err := sql.Open(driver, dsn)

	var versionString string

	err = db.QueryRow("show server_version").Scan(&versionString)
	if err != nil {
		db.Close()
		return nil, errors.Wrap(err, "Failed to query PostgreSQL version.")
	}
	log.Info("Connected to PostgreSQL version %s", versionString)

	version, err := ParseVersion(versionString)
	if err != nil {
		db.Close()
		return nil, errors.Errorf("Failed to parse PostgreSQL version %s", versionString)
	}
	if version.Major < 10 && version.Minor < 5 {
		db.Close()
		return nil, errors.New("PostgreSQL version 9.5 or newer is required.")
	}

	return &PgDB{
		DB: db,
	}, err
}

func ConfigureManaged(dataDirectory string) (*PostgresManager, error) {

	version, err := GetVersion()
	if err != nil {
		return nil, err
	}

	log.Info("Found PostgreSQL version %s (%s)", version.MajorMinor,
		version.Full)

	if version.Major < 10 && version.Minor < 5 {
		return nil, errors.New("PostgreSQL version 9.5 or newer required.")
	}

	manager, err := NewPostgresManager(dataDirectory)
	if err != nil {
		return nil, err
	}
	if !manager.IsInitialized() {
		log.Info("Initializing %s", dataDirectory)
		manager.Init()
	}

	return manager, nil
}

func ManagedConfig(dataDirectory string) (PgConfig, error) {
	pgConfig := PgConfig{}

	absPath, err := filepath.Abs(dataDirectory)
	if err != nil {
		return pgConfig, err
	}

	pgVersion, err := GetVersion()
	if err != nil {
		return pgConfig, err
	}

	pgConfig.Host = path.Join(
		absPath, fmt.Sprintf("pgdata%s", pgVersion.MajorMinor))
	pgConfig.Database = PGDATABASE
	pgConfig.User = PGUSER
	pgConfig.Password = PGPASS

	return pgConfig, nil
}

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
)

const PGDATABASE = "evebox"
const PGUSER = "evebox"
const PGPASS = "evebox"
const PGPORT = "8432"

type Service struct {
	db *sql.DB
}

func NewService() (*Service, error) {
	args := fmt.Sprintf(
		"dbname=%s user=%s password=%s port=%s sslmode=%s",
		PGDATABASE,
		PGUSER,
		PGPASS,
		PGPORT,
		"disable")
	db, err := sql.Open("postgres", args)
	if err != nil {
		log.Fatal(err)
	}

	var pgVersion string

	err = db.QueryRow("select version()").Scan(&pgVersion)
	if err != nil {
		return nil, err
	}
	log.Info("Connected to PostgreSQL version %s.", pgVersion)

	return &Service{
		db: db,
	}, nil
}

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

package auth

import (
	"context"
	"github.com/jasonish/evebox/log"
	"golang.org/x/oauth2"
	"golang.org/x/oauth2/github"
	"io/ioutil"
	"net/http"
)

type Github struct {
	config *oauth2.Config
}

func NewGithub() *Github {
	config := &oauth2.Config{
		ClientID:     "23df7c7bd97d345d6001",
		ClientSecret: "c4dc3cbefb80c44d0f93a7a0a181ef0ea82937cb",
		RedirectURL:  "http://localhost:5636/auth/github/callback",
		Endpoint:     github.Endpoint,
		Scopes:       []string{"read:org"},
	}
	return &Github{
		config: config,
	}
}

func (g *Github) Handler(w http.ResponseWriter, r *http.Request) {
	http.Redirect(w, r, g.config.AuthCodeURL("secret"),
		http.StatusTemporaryRedirect)
}

func (g *Github) Callback(w http.ResponseWriter, r *http.Request) {
	code := r.FormValue("code")
	secret := r.FormValue("state")
	log.Info("Code: %s; state: %s", code, secret)

	token, err := g.config.Exchange(context.Background(), code)
	if err != nil {
		log.Error("exchange failed: %v", err)
		return
	}
	log.Info("Token: %v", token)

	client := g.config.Client(context.Background(), token)

	response, err := client.Get("https://api.github.com/user")
	if err != nil {
		log.Error("Failed to get client info: %v", err)
		return
	}
	defer response.Body.Close()
	data, _ := ioutil.ReadAll(response.Body)
	log.Info("User: %s", string(data))

	response, err = client.Get("https://api.github.com/user/orgs")
	if err != nil {
		log.Error("Failed to get client info: %v", err)
		return
	}
	defer response.Body.Close()
	data, _ = ioutil.ReadAll(response.Body)
	log.Info("Orgs: %s", string(data))

}

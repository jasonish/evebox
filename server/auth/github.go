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
	"encoding/json"
	"github.com/jasonish/evebox/appcontext"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/server/sessions"
	"github.com/pkg/errors"
	"golang.org/x/oauth2"
	"golang.org/x/oauth2/github"
	"net/http"
)

const userUrl = "https://api.github.com/user"
const orgsUrl = "https://api.github.com/user/orgs"

type GitHubAuthenticator struct {
	oauthConfig  *oauth2.Config
	SessionStore *sessions.SessionStore
}

type GitHubUser struct {
	Login            string `json:"login"`
	Id               int64  `json:"id"`
	OrganizationsUrl string `json:"organizations_url"`
	Name             string `json:"name"`
	Email            string `json:"email"`
}

type GitHubOrg struct {
	Login string `json:"login"`
}

func NewGithub(config appcontext.GithubAuthConfig) *GitHubAuthenticator {
	if config.ClientID == "" {
		log.Fatal("GitHub client ID required")
	}
	if config.ClientSecret == "" {
		log.Fatal("GitHub client secret required")
	}
	if config.Callback == "" {
		log.Fatal("GitHub callback URL required")
	}
	oauthConfig := &oauth2.Config{
		ClientID:     config.ClientID,
		ClientSecret: config.ClientSecret,
		RedirectURL:  config.Callback,
		Endpoint:     github.Endpoint,
		Scopes:       []string{"read:org"},
	}
	return &GitHubAuthenticator{
		oauthConfig: oauthConfig,
	}
}

func (g *GitHubAuthenticator) Handler(w http.ResponseWriter, r *http.Request) {
	session := g.SessionStore.FindSession(r)
	if session == nil {
		log.Info("Creating session for GitHub authentication request.")
		session = &sessions.Session{
			Id: g.SessionStore.GenerateID(),
		}
		g.SessionStore.Put(session)
		w.Header().Set(g.SessionStore.Header, session.Id)
	}
	log.Info("GitHubAuthenticator.Handler: session: %s", session.String())

	encoder := json.NewEncoder(w)
	encoder.Encode(map[string]interface{}{
		"redirect": g.oauthConfig.AuthCodeURL(session.Id),
	})
}

func (g *GitHubAuthenticator) Callback(w http.ResponseWriter, r *http.Request) {
	code := r.FormValue("code")
	state := r.FormValue("state")
	log.Info("Code: %s; state: %s", code, state)

	// Find session by the state parameter.
	session, _ := g.SessionStore.Get(state)
	if session == nil {
		log.Warning("Did not find session for oauth callback.")
		return
	}
	log.Info("Found session for GitHub callback: %s", session)

	token, err := g.oauthConfig.Exchange(context.Background(), code)
	if err != nil {
		log.Error("exchange failed: %v", err)
		return
	}

	if !token.Valid() {
		log.Warning("Received invalid GitHub token: %s",
			token.TokenType)
		w.Write([]byte("Github login failed."))
		w.WriteHeader(http.StatusUnauthorized)
		g.SessionStore.DeleteById(state)
		return
	}

	client := g.oauthConfig.Client(context.Background(), token)

	githubUser, err := g.GetUser(client)
	if err != nil {
		log.Error("Failed to get user details: %v", err)
	}
	log.Println(githubUser)

	orgs, err := g.GetOrgs(client)
	if err != nil {
		log.Error("Failed to get user organizations: %v", err)
	}
	log.Println(orgs)

	// Looks like a successful login. Will redirect.
	http.Redirect(w, r, "/", http.StatusTemporaryRedirect)
	return
}

func (g *GitHubAuthenticator) GetOrgs(client *http.Client) ([]GitHubOrg, error) {
	orgs := make([]GitHubOrg, 0)
	return orgs, g.fetchAndDecode(client, orgsUrl, &orgs)
}

func (g *GitHubAuthenticator) GetUser(client *http.Client) (GitHubUser, error) {
	var githubUser GitHubUser
	return githubUser, g.fetchAndDecode(client, userUrl, &githubUser)
}

func (g *GitHubAuthenticator) fetchAndDecode(client *http.Client, url string, result interface{}) error {
	response, err := client.Get(url)
	if err != nil {
		return errors.Wrap(err, "request failed")
	}
	decoder := json.NewDecoder(response.Body)
	decoder.UseNumber()
	err = decoder.Decode(result)
	if err != nil {
		return errors.Wrap(err, "failed to decode response")
	}
	return nil
}

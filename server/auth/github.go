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
	"fmt"
	"github.com/jasonish/evebox/appcontext"
	"github.com/jasonish/evebox/core"
	"github.com/jasonish/evebox/log"
	"github.com/jasonish/evebox/server/sessions"
	"github.com/pkg/errors"
	"golang.org/x/oauth2"
	"golang.org/x/oauth2/github"
	"net/http"
)

const gitHubUserUrl = "https://api.github.com/user"

type GitHubAuthenticator struct {
	oauthConfig  *oauth2.Config
	SessionStore *sessions.SessionStore
	userStore    core.UserStore
}

type GitHubUser struct {
	Login            string `json:"login"`
	Id               int64  `json:"id"`
	OrganizationsUrl string `json:"organizations_url"`
	Name             string `json:"name"`
	Email            string `json:"email"`
}

func NewGithub(config appcontext.GithubAuthConfig, userstore core.UserStore) *GitHubAuthenticator {
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
	}
	return &GitHubAuthenticator{
		oauthConfig: oauthConfig,
		userStore:   userstore,
	}
}

func (g *GitHubAuthenticator) Handler(w http.ResponseWriter, r *http.Request) {
	session := g.SessionStore.FindSession(r)
	if session == nil {
		log.Info("Creating session for GitHub authentication request.")
		session = g.SessionStore.NewSession()
		g.SessionStore.Put(session)
		w.Header().Set(g.SessionStore.Header, session.Id)
	}

	if r.FormValue("fail-redirect") != "" {
		session.Other["github-fail-redirect"] =
			r.FormValue("fail-redirect")
	}

	if r.FormValue("success-redirect") != "" {
		session.Other["github-success-redirect"] =
			r.FormValue("success-redirect")
	}

	encoder := json.NewEncoder(w)
	encoder.Encode(map[string]interface{}{
		"redirect": g.oauthConfig.AuthCodeURL(session.Id),
	})
}

func handleError(w http.ResponseWriter, r *http.Request, session *sessions.Session, status int, message string) {
	if session != nil {
		redirectUrl, ok := session.Other["github-fail-redirect"].(string)
		if ok {
			redirectUrl = fmt.Sprintf("%s;error=%s", redirectUrl,
				message)
			http.Redirect(w, r, redirectUrl, http.StatusTemporaryRedirect)
			return
		}
	}

	w.Header().Set("Content-Type", "text/plain")
	w.WriteHeader(status)
	w.Write([]byte(message))
}

func (g *GitHubAuthenticator) Callback(w http.ResponseWriter, r *http.Request) {
	code := r.FormValue("code")
	state := r.FormValue("state")

	// Find session by the state parameter.
	session := g.SessionStore.Get(state)
	if session == nil {
		log.Error("Did not find session for oauth callback.")
		handleError(w, r, nil, http.StatusUnauthorized, "No session for GitHub authentication.")
		return
	}

	token, err := g.oauthConfig.Exchange(context.Background(), code)
	if err != nil {
		log.Error("GitHub exchange failed: %v", err)
		handleError(w, r, session, http.StatusUnauthorized, "GitHub exchange failed.")
		g.SessionStore.Delete(session)
		return
	}

	if !token.Valid() {
		log.Error("Invalid GitHub Oauth2 token: %v", token)
		handleError(w, r, session, http.StatusBadRequest, "Login failed: Received bad token from GitHub.")
		g.SessionStore.Delete(session)
		return
	}

	client := g.oauthConfig.Client(context.Background(), token)

	githubUser, err := g.GetUser(client)
	if err != nil {
		log.Error("Failed to fetch user details from GitHub: %v", err)
		handleError(w, r, session, http.StatusBadRequest,
			"Failed to fetch user details from GitHub")
		return
	}

	user, err := g.userStore.FindByGitHubUsername(githubUser.Login)
	if err != nil {
		log.Error("GitHub user %s does not exist in local database", githubUser.Login)
		handleError(w, r, session, http.StatusUnauthorized,
			"Access denied - GitHub user does not exist in local database.")
		return
	}
	session.User = user
	session.RemoteAddr = r.RemoteAddr

	log.Info("User %s logged in (via GitHub) from %s", user.Username,
		r.RemoteAddr)

	redirectUrl, ok := session.Other["github-success-redirect"].(string)
	if ok {
		http.Redirect(w, r, redirectUrl, http.StatusTemporaryRedirect)
		return
	}

	log.Warning("GitHub login successful, but no success redirect URL.")
	w.WriteHeader(http.StatusOK)
}

func (g *GitHubAuthenticator) GetUser(client *http.Client) (GitHubUser, error) {
	var githubUser GitHubUser
	return githubUser, g.fetchAndDecode(client, gitHubUserUrl, &githubUser)
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

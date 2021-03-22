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

import {AfterViewInit, Component, OnInit} from "@angular/core";
import {ActivatedRoute, Router} from "@angular/router";
import {ApiService} from "../api.service";
import {ClientService} from "../client.service";

declare var window: any;

@Component({
    templateUrl: "login.component.html",
})
export class LoginComponent implements OnInit, AfterViewInit {

    model: any = {
        username: "",
        password: "",
    };

    username = false;
    password = false;

    error: string;

    loginMessage: string;

    constructor(private api: ApiService,
                private router: Router,
                private route: ActivatedRoute,
                private client: ClientService) {
    }

    ngOnInit() {

        if (this.route.snapshot.params["error"]) {
            this.error = this.route.snapshot.params["error"];
        }

        console.log(this.route.snapshot.params);

        // Get the login types.
        this.api._options("/api/1/login")
            .toPromise()
            .then((options) => {
                console.log("Login options:");
                console.log(options);
                if (options.authentication.required) {
                    for (let authType of options.authentication.types) {
                        switch (authType) {
                            case "username":
                                this.username = true;
                                break;
                            case "usernamepassword":
                                this.username = true;
                                this.password = true;
                                break;
                        }
                    }
                }
                if (options.login_message) {
                    this.loginMessage = options.login_message;
                }
            });

        this.focus();
    }

    ngAfterViewInit() {
        this.focus();
    }

    focus() {
        let em = document.getElementById("username");
        if (em) {
            em.focus();
            document.execCommand("selectall", null, "");
        }
    }

    login() {
        console.log("Calling api.login...");
        this.api.login(this.model.username, this.model.password)
            .then(() => {
                console.log("Login successful, redirecting to /");
                this.router.navigate(["/"]);
            })
            .catch(error => {
                console.log(`Login failed:`);
                console.log(error);
                if (error.status === 401) {
                    this.error = "Login failed";
                }
                else {
                    this.error = "Login failed: " + JSON.stringify(error);
                }
            });
    }
}

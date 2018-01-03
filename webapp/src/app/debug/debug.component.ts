/* Copyright (c) 2018 Jason Ish
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

import {Component, OnInit} from "@angular/core";
import {ClientService} from "../client.service";

@Component({
    selector: "app-debug",
    templateUrl: "./debug.component.html",
    styleUrls: ["./debug.component.scss"]
})
export class DebugComponent implements OnInit {

    public loginModel:any = {
        username: "",
        password: "",
    };

    constructor(private client: ClientService) {
    }

    ngOnInit() {
    }

    checkAuthentication() {
        this.client.checkAuthentication()
            .subscribe(
                (response) => {
                    console.log(response);
                }
            );
    }

    logout() {
        this.client.logout()
            .subscribe((response) => {
                console.log("logout ok; response:");
                console.log(response);
            }, (error) => {
                console.log("logout error:");
                console.log(error);
            });
    }

    login() {
        console.log(`Logging in: username=${this.loginModel.username}; password=${this.loginModel.password}`);
        this.client.login(this.loginModel.username, this.loginModel.password)
            .subscribe(
                (response: any) => {
                    console.log("login ok; response:");
                    console.log(response);
                },
                (error: any) => {
                    console.log("login failed; error:");
                    console.log(error);
                }
            );
    }

}

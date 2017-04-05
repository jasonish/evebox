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

import {Component, OnInit} from '@angular/core';
import {Router} from '@angular/router';
import {AppService} from './app.service';

@Component({
    template: `<div class="row">
    <div class="col-md-4"></div>
    <div class="col-md-4">
        <h3 style="text-align: center;">EveBox Login</h3>
        <div class="jumbotron">
            <form>
                <div *ngIf="loginMessage">
                    {{loginMessage}}
                    <br/>
                    <br/>
                </div>

                <input type="text" autofocus class="form-control"
                       [(ngModel)]="username" name="username"
                       placeholder="Username">
                <br/>
                <button type="submit" class="btn btn-primary btn-block"
                        (click)="login()">Login
                </button>
            </form>
        </div>
    </div>
    <div class="col-md-4"></div>
</div>
`,
})
export class LoginComponent implements OnInit {

    username: string = "";

    loginMessage: string;

    constructor(private appService: AppService,
                private router: Router) {
    }

    ngOnInit() {
        if (true == true) {
            return;
        }
        this.appService.checkAuthenticated().then((response) => {
            console.log("LoginComponent.ngOnInit: Already authenticated, redirecting to /.");
            this.router.navigate(['/']);
        }, (error) => {
            this.loginMessage = error["login_message"];
            console.log("failed to get config");
            console.log(error);
        })
    }

    login() {
        console.log("Logging in user " + this.username);
        this.appService.login(this.username).then(response => {
            console.log("Login successful, navigating to /");
            this.router.navigate(['/']);
        }, (error) => {
            console.log("Failed to login...");
        });
    }
}

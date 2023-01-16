// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import { Alert, Button, Col, Container, Form, Row } from "solid-bootstrap";
import {
  createEffect,
  createResource,
  createSignal,
  Show,
  Suspense,
} from "solid-js";
import { createStore } from "solid-js/store";
import { useNavigate, useSearchParams } from "@solidjs/router";
import { LoginOptions } from "./api";
import * as API from "./api";

async function getLoginOptions(): Promise<LoginOptions> {
  let response = await fetch("/api/1/login", {
    method: "get",
  });
  const json = await response.json();
  return json;
}

export const Login = () => {
  const [loginForm, setLoginForm] = createStore({
    username: "",
    password: "",
  });
  const [error, setError] = createSignal(false);

  const [searchParams, setSearchParams] = useSearchParams();
  const [loginOptions] = createResource(getLoginOptions);
  const navigate = useNavigate();

  const doLogin = async (e: any) => {
    e.preventDefault();

    API.login(loginForm.username, loginForm.password)
      .then(() => {
        navigate(searchParams.redirectTo || "/inbox");
      })
      .catch(() => {
        setError(true);
      });
  };

  const isValid = () => {
    switch (loginOptions()?.authentication.type) {
      case "username":
        return loginForm.username.length > 0;
      case "usernamepassword":
        return loginForm.username.length > 0 && loginForm.password.length > 0;
      default:
        return true;
    }
  };

  createEffect(() => {
    if (loginOptions()?.authentication.type === "anonymous") {
      navigate(searchParams.redirectTo || "/inbox");
    }
  });

  return (
    <>
      <Container class={"mt-5"}>
        <Row>
          <Col></Col>

          <Col xs={12} md={8} lg={6}>
            <Show when={error()}>
              <Alert dismissible variant={"danger"}>
                Login Failed
              </Alert>
            </Show>

            <div class={"bg-theme"} style={"padding: 20px"}>
              <Suspense>
                {loginOptions() && (
                  <Form onsubmit={doLogin}>
                    <Form.Group>
                      <Form.Label>Username:</Form.Label>
                      <Form.Control
                        type={"text"}
                        spellcheck={false}
                        oninput={(e) =>
                          setLoginForm("username", e.currentTarget.value)
                        }
                        placeholder={"Username..."}
                      />
                    </Form.Group>

                    {loginOptions()?.authentication.type ===
                      "usernamepassword" && (
                      <Form.Group class={"mt-3"}>
                        <Form.Label>Password:</Form.Label>
                        <Form.Control
                          oninput={(e) =>
                            setLoginForm("password", e.currentTarget.value)
                          }
                          type={"password"}
                          placeholder={"Password..."}
                        />
                      </Form.Group>
                    )}

                    <div class={"d-grid mt-3"}>
                      <Button
                        class={""}
                        variant={"primary"}
                        type={"submit"}
                        disabled={!isValid()}
                      >
                        Login
                      </Button>
                    </div>
                  </Form>
                )}
              </Suspense>
            </div>
          </Col>
          <Col></Col>
        </Row>
      </Container>
    </>
  );
};
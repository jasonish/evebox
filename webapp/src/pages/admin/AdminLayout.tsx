// SPDX-FileCopyrightText: (C) 2026 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { A } from "@solidjs/router";
import { JSX, Show } from "solid-js";
import { Top } from "../../Top";
import { distributionName, serverConfig } from "../../config";

// Shared frame for the admin pages: the top navbar plus a sidebar
// navigating between the admin sections, with the active section
// highlighted. On small screens the sidebar becomes a horizontal pill
// bar above the content.
export function AdminLayout(props: { children?: JSX.Element }) {
  return (
    <>
      <Top disableRange={true} />
      <div class="container app-admin-container my-3">
        <div class="row g-3">
          <nav class="col-md-3 col-lg-2" aria-label="Administration">
            <div class="app-admin-nav-title text-uppercase text-body-secondary fw-semibold mb-2">
              Administration
            </div>
            <ul class="nav nav-pills flex-md-column gap-1 app-admin-nav">
              <AdminNavLink href="/admin" end={true} label="General" />
              <Show when={serverConfig()?.datastore === "elasticsearch"}>
                <AdminNavLink
                  href="/admin/elastic"
                  label={distributionName()}
                />
              </Show>
              <AdminNavLink href="/admin/agents" label="Agents" />
              <AdminNavLink href="/admin/filters" label="Filters" />
            </ul>
          </nav>
          <main class="col-md-9 col-lg-10">{props.children}</main>
        </div>
      </div>
    </>
  );
}

function AdminNavLink(props: { href: string; label: string; end?: boolean }) {
  return (
    <li class="nav-item">
      <A
        href={props.href}
        end={props.end}
        class="nav-link"
        activeClass="active"
      >
        {props.label}
      </A>
    </li>
  );
}

// Consistent heading for admin pages: a title, an optional one-line
// description, and an optional right side for page tools such as a
// search input.
export function AdminPageHeader(props: {
  title: string;
  subtitle?: string;
  children?: JSX.Element;
}) {
  return (
    <div class="d-flex flex-wrap align-items-center justify-content-between gap-2 border-bottom pb-2 mb-3">
      <div>
        <h4 class="mb-0">{props.title}</h4>
        <Show when={props.subtitle}>
          <div class="text-body-secondary small">{props.subtitle}</div>
        </Show>
      </div>
      {props.children}
    </div>
  );
}

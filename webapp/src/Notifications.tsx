// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

import { For } from "solid-js";
import { createSignal } from "solid-js";
import { Toast } from "solid-bootstrap";

const DEFAULT_DELAY = 3000;

let notificationId = 0;

interface Notification {
  id: number;
  message: string;
  delay: number;
}

const [notifications, setNotifications] = createSignal<Notification[]>([]);

export function addNotification(message: string, options?: { delay?: number }) {
  setNotifications((l: any) => [
    {
      id: notificationId++,
      message: message,
      delay: options?.delay || DEFAULT_DELAY,
    },
    ...l,
  ]);
}

export function removeNotification(e: any) {
  setNotifications((l) => {
    const index = l.indexOf(e);
    if (index > -1) {
      l.splice(index, 1);
    }
    return [...l];
  });
}

export function Notifications() {
  return (
    <div class="toast-container position-fixed top-0 end-0 p-3">
      <For each={notifications()}>
        {(e) => (
          <>
            <Toast
              bg={"info"}
              autohide
              delay={e.delay}
              onClose={() => {
                removeNotification(e);
              }}
            >
              <Toast.Header>Info</Toast.Header>
              <Toast.Body>{e.message}</Toast.Body>
            </Toast>
          </>
        )}
      </For>
    </div>
  );
}

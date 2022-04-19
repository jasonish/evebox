// Copyright (C) 2014-2022 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import { Component, OnInit } from '@angular/core';
import {NgbActiveModal} from "@ng-bootstrap/ng-bootstrap";

@Component({
  selector: 'app-help',
  templateUrl: './help.component.html',
  styleUrls: ['./help.component.scss']
})
export class HelpComponent implements OnInit {

  active = 1;

  shortcuts: any[] = [
    {
      shortcut: "?",
      help: "Show help."
    },

    {
      shortcut: "g i",
      help: "Goto inbox."
    },
    {
      shortcut: "g x",
      help: "Goto escalated."
    },
    {
      shortcut: "g a",
      help: "Goto alerts."
    },
    {
      shortcut: "g e",
      help: "Goto events."
    },

    {
      shortcut: "F8",
      help: "In inbox, archives active alert."
    },

    {
      shortcut: "F9",
      help: "In inbox, escalate and archive active alert."
    },

    {
      shortcut: "e",
      help: "Archive selected alerts."
    },
    {
      shortcut: "s",
      help: "Toggles escalated status of alert."
    },
    {
      shortcut: "x",
      help: "Select highlighted event."
    },
    {
      shortcut: "/",
      help: "Focus search input."
    },
    {
      shortcut: "j",
      help: "Next event."
    },
    {
      shortcut: "k",
      help: "Previous event."
    },
    {
      shortcut: "o",
      help: "Open event."
    },
    {
      shortcut: "u",
      help: "When in event view, go back to event listing."
    },
    {
      shortcut: "* a",
      help: "Select all alerts in view.",
    },
    {
      shortcut: "* n",
      help: "Deselect all alerts.",
    },
    {
      shortcut: "* 1",
      help: "Select all alerts with same SID as current alert.",
    },
    {
      shortcut: ".",
      help: "Dropdown alert menu.",
    },
  ];


  constructor(public activeModal: NgbActiveModal) { }

  ngOnInit(): void {
  }

}

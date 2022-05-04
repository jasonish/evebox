// Copyright (C) 2020-2022 Jason Ish <jason@codemonkey.net>
//
// SPDX-License-Identifier: MIT

import { trigger, state, style, transition, animate } from '@angular/animations';

export let loadingAnimation = trigger('loadingState', [
        state('false', style({
            opacity: 1.0,
        })),
        state('true', style({
            opacity: 0.5,
        })),
        transition('false => true', animate('500ms')),
        transition('true => false', animate('500ms'))
    ]
);

// Animation for the loading spinner.
export let spinningLoaderAnimation = trigger('eveboxSpinningLoaderAnimation', [
        state('void', style({
            opacity: 0,
        })),
        state('*', style({
            opacity: 1,
        })),
        transition('void => *', animate('500ms')),
        transition('* => void', animate('500ms')),
    ]
);

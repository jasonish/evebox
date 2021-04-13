// Copyright (C) 2020 Jason Ish
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND,
// EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
// MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT.
// IN NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
// DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
// OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE
// OR OTHER DEALINGS IN THE SOFTWARE.

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

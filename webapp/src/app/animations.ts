// Copyright (C) 2020 Jason Ish
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU Affero General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU Affero General Public License for more details.
//
// You should have received a copy of the GNU Affero General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

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

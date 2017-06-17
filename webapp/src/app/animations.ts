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

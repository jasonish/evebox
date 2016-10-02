import {style, animate, state, transition, trigger} from "@angular/core";

export var loadingAnimation = trigger('loadingState', [
        state('false', style({
            opacity: '1.0',
        })),
        state('true', style({
            opacity: '0.5',
        })),
        transition('false => true', animate('1000ms')),
        transition('true => false', animate('1000ms'))
    ]
);

import {Pipe, PipeTransform} from '@angular/core';

@Pipe({
    name: 'genericPrettyPrinter'
})
export class EveBoxGenericPrettyPrinter implements PipeTransform {

    transform(value: any, args: any): any {

        // Replace underscores with spaces.
        value = value.replace(/_/g, ' ');

        // Captialize the first letter of each word.
        value = value.toLowerCase().replace(/\b./g, (a: any) => {
            return a.toUpperCase();
        });

        return value;
    }

}
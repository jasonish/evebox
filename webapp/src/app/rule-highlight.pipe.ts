import {Pipe, PipeTransform} from '@angular/core';

@Pipe({
    name: 'ruleHighlight'
})
export class RuleHighlightPipe implements PipeTransform {

    transform(value: any, args?: any): any {

        value = value.replace(
                /^([^\s]+)\s+([^\s]+)\s+([^\s]+)\s+([^\s]+)\s+([^\s]+)\s+([^\s]+)\s+([^\s]+)\s+/,
                `<span class="rule-header-action">$1</span> 
                 <span class="rule-header-proto">$2</span>
                 <span class="rule-header-addr">$3</span>
                 <span class="rule-header-port">$4</span> 
                 <span class="rule-header-direction">$5</span> 
                 <span class="rule-header-addr">$6</span>
                 <span class="rule-header-port">$7</span> `);

        value = value.replace(/:([^;]+)/g, `:<span class="rule-keyword-value">$1</span>`);
        value = value.replace(/(\w+\:)/g, `<span class="rule-keyword">$1</span>`);


        return value;
    }

}

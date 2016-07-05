import {Pipe, PipeTransform} from "@angular/core";
@Pipe({
    name: "eveboxBase64Decode"
})
export class EveboxBase64DecodePipe implements PipeTransform {

    transform(value:any, args:any):any {
        return atob(value);
    }

}
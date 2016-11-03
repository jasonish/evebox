import {Injectable} from "@angular/core";
import {Http, Response} from "@angular/http";

@Injectable()
export class ApiService {

    private baseUrl:string = window.location.pathname;

    constructor(private http:Http) {
    }

    post(path:string, body:any) {
        return this.http.post(this.baseUrl + path, JSON.stringify(body))
            .map((res:Response) => res.json())
            .toPromise();
    }

    get(path:string):Promise<any> {
        return this.http.get(`${this.baseUrl}${path}`)
            .map((res:Response) => res.json())
            .toPromise();
    }

    getVersion() {
        return this.http.get(this.baseUrl + "api/1/version")
            .map((res:Response) => res.json())
            .toPromise();
    }

    eventToPcap(what:any, event:any) {

        let form = document.createElement("form");
        form.setAttribute("method", "post");
        form.setAttribute("action", "api/1/eve2pcap");

        let whatField = document.createElement("input");
        whatField.setAttribute("type", "hidden");
        whatField.setAttribute("name", "what");
        whatField.setAttribute("value", what);
        form.appendChild(whatField);

        let eventField = document.createElement("input");
        eventField.setAttribute("type", "hidden");
        eventField.setAttribute("name", "event");
        eventField.setAttribute("value", JSON.stringify(event));
        form.appendChild(eventField);

        document.body.appendChild(form);
        form.submit();
    }

}
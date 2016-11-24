import {Injectable} from "@angular/core";
import {Http, Response} from "@angular/http";

export class QueryStringBuilder {

    keys:any = {};

    set(key:string, value:any) {
        this.keys[key] = value;
    }

    build() {
        let parts:any = [];

        for (let key in this.keys) {
            parts.push(`${key}=${this.keys[key]}`);
        }

        return parts.join("&")
    }
}

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

    postRaw(path:string, body:any) {
        return this.http.post(this.baseUrl + path, body)
            .map((res:Response) => res.json())
            .toPromise();
    }

    get(path:string, options = {}):Promise<any> {
        return this.http.get(`${this.baseUrl}${path}`, options)
            .map((res:Response) => res.json())
            .toPromise();
    }

    getWithParams(path:string, params={}):Promise<any> {

        let qsb:any = [];

        for (let param in params) {
                    qsb.push(`${param}=${params[param]}`);
        }

        return this.get(`${path}?${qsb.join("&")}`);

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

    reportHistogram(options:ReportHistogramOptions = {}) {
        let query:any = [];

        if (options.timeRange && options.timeRange > 0) {
            query.push(`timeRange=${options.timeRange}s`);
        }

        if (options.interval) {
            query.push(`interval=${options.interval}`);
        }

        if (options.addressFilter) {
            query.push(`addressFilter=${options.addressFilter}`);
        }

        if (options.queryString) {
            query.push(`queryString=${options.queryString}`);
        }

        if (options.sensorFilter) {
            query.push(`sensorFilter=${options.sensorFilter}`);
        }

        if (options.dnsType) {
            query.push(`dnsType=${options.dnsType}`);
        }

        return this.get(`api/1/report/histogram?${query.join("&")}`);
    }

    reportAgg(agg:string, options:ReportAggOptions = {}) {

        let qsb:any = [];

        qsb.push(`agg=${agg}`);

        for (let option in options) {
            switch (option) {
                case "timeRange":
                    if (options[option] > 0) {
                        qsb.push(`timeRange=${options[option]}s`);
                    }
                    break;
                default:
                    qsb.push(`${option}=${options[option]}`);
                    break;
            }
        }

        return this.get(`api/1/report/agg?${qsb.join("&")}`);
    }

}

export interface ReportHistogramOptions {
    timeRange?:number
    interval?:string
    addressFilter?:string
    queryString?:string
    sensorFilter?:string
    eventType?:string
    dnsType?:string
}

// Options for an aggregation report.
export interface ReportAggOptions {
    size?:number
    queryString?:string
    timeRange?:string

    // Event type.
    eventType?:string

    // Subtype info.
    dnsType?:string

}
import {Injectable} from "@angular/core";
import moment = require("moment");
import UnitOfTime = moment.UnitOfTime;


@Injectable()
export class TopNavService  {

    timeRange:string = "24h";

    /**
     * Get the time range in seconds.
     */
    getTimeRangeAsSeconds():any {
        if (this.timeRange == "") {
            // Everything...
            return 0;
        }
        let parts:any[] = <any[]>this.timeRange.match(/(\d+)(\w+)/);
        let value:number = parseInt(parts[1]);
        let unit:string = parts[2];
        return moment.duration(value, <UnitOfTime>unit).asSeconds();
    }
}
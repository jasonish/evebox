import {inject, TestBed} from "@angular/core/testing";

import {ClientService} from "./client.service";
import {HttpClientModule} from "@angular/common/http";

describe("ClientService", () => {
    beforeEach(() => {
        TestBed.configureTestingModule({
            providers: [ClientService],
            imports: [
                HttpClientModule,
            ]
        });
    });

    it("should be created", inject([ClientService], (service: ClientService) => {
        expect(service).toBeTruthy();
    }));
});

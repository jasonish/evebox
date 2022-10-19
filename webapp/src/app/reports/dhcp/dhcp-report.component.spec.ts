import { ComponentFixture, TestBed } from "@angular/core/testing";

import { DhcpReportComponent } from "./dhcp-report.component";

describe("DhcpComponent", () => {
    let component: DhcpReportComponent;
    let fixture: ComponentFixture<DhcpReportComponent>;

    beforeEach(async () => {
        await TestBed.configureTestingModule({
            declarations: [DhcpReportComponent],
        }).compileComponents();
    });

    beforeEach(() => {
        fixture = TestBed.createComponent(DhcpReportComponent);
        component = fixture.componentInstance;
        fixture.detectChanges();
    });

    it("should create", () => {
        expect(component).toBeTruthy();
    });
});

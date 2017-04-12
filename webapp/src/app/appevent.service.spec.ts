import { TestBed, inject } from '@angular/core/testing';

import { AppEventService } from './appevent.service';

describe('AppEventService', () => {
  beforeEach(() => {
    TestBed.configureTestingModule({
      providers: [AppEventService]
    });
  });

  it('should ...', inject([AppEventService], (service: AppEventService) => {
    expect(service).toBeTruthy();
  }));
});

// SPDX-FileCopyrightText: (C) 2023 Jason Ish <jason@codemonkey.net>
// SPDX-License-Identifier: MIT

export function scrollToClass(className: string, index: number) {
  let error = 175;

  // If first element, just scroll to the top. Assumes that the rows start near the top of the page.
  if (index === 0) {
    scrollTo(0, 0);
  }

  const classElements = document.getElementsByClassName(className);

  if (index > 0 && classElements.length < index - 1) {
    index = classElements.length - 1;
  }

  let el = classElements[index] as HTMLElement;
  if (el === undefined) {
    return;
  }
  let elOffset = el.offsetTop;
  let elHeight = el.scrollHeight;
  let windowOffset = window.scrollY;
  let windowHeight = window.innerHeight;

  let elBottom = elOffset + elHeight;
  let windowBottom = windowOffset + windowHeight;

  if (elBottom > windowBottom - error - windowHeight * 0.2) {
    let newOffset = windowOffset + elHeight;

    // The first case gives us somewhat of a smooth scroll. But if it
    // doesn't get the active element into view, we need to jump to
    // which is handled by the else.
    if (elBottom < newOffset + windowHeight) {
      window.scrollTo(0, windowOffset + elHeight);
    } else {
      window.scrollTo(0, elBottom);
    }
  } else if (windowOffset > 0) {
    if (elOffset < windowOffset + windowHeight * 0.1) {
      window.scrollTo(0, Math.min(elOffset, windowOffset - elHeight));
    }
  }
}

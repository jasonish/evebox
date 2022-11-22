export function indexOf(array: any, what: any): number {
  if (array && Array.isArray(array)) {
    return array.indexOf(what);
  } else {
    return -1;
  }
}

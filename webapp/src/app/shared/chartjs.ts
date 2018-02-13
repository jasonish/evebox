import * as palette from "google-palette";

export function getColourPalette(count: number): string[] {
    let colours = palette("qualitative", count);
    return colours.map(colour => {
        return "#" + colour;
    });
}
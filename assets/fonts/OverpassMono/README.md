# Overpass Mono

Unmodified **Overpass Mono Regular** is embedded as the primary face for both
egui text families. Menus, headings, tabs, buttons, section cards, graphs and
numeric editors therefore share the owner's requested monospaced typography.
The existing egui font chains remain after it for Unicode fallback.

Source: https://github.com/RedHatOfficial/Overpass
Pinned upstream revision: `c580d28bfab7f39013568e684a65eeb23eff588d`.
Files: `fonts/ttf_mono/OverpassMono-Regular.ttf` and root `OFL.txt`.

Copyright 2021 The Overpass Project Authors. Distributed under the supplied,
complete SIL Open Font License 1.1. Do not omit `OFL.txt` from distributions.
No font conversion, subsetting, renaming or glyph modifications were performed.
The bundled OFL.txt has one upstream trailing space removed; license wording is unchanged.

SHA-256:

| File | Hash |
| --- | --- |
| OverpassMono-Regular.ttf | `8ae8acc4e4c0febde3dfd26a5e5ff116e3ca3eab84ca3049d3f6f24bd7ae8bbb` |
| OFL.txt | `a5ee96d5e287afc15cf4343be02e93cf678c285e52367658781cb5ab198ffd13` |

Sizes are logical egui points (body/buttons/numbers 13, small/rulers 11,
headings 17). Windows monitor DPI and the existing custom UI zoom remain
independent; the font is never installed into the user's operating system.

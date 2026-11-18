# Bitmap font (contact-sheet labels)

Owner: WP 2.3. Used by `crates/frap-render/src/composite/font.rs` (embedded with
`include_str!`) to write view names on `frap.render_contact_sheet` tiles.

| File | Source | Licence |
|---|---|---|
| `frap5x7.txt` | Drawn for frap (5 x 7 cells: `A`–`Z`, `0`–`9`, space and `-_.,:()/+=#'?`) | GPL-3.0-or-later (frap) |

Format: a `glyph <c>` line (`glyph space` for a space) followed by 7 rows of 5 cells, `#` for
ink and `.` for background; `#` lines at the top of the file are comments. Lowercase letters
draw as their uppercase glyph; other missing characters draw as `?`. Glyphs advance 6 cells
(one blank column), lines are 9 cells high, and the compositor scales cells to whole pixels.

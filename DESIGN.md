---
name: OpenCode TUI Design System (extracted)
source: references/opencode/packages/tui/src/theme/assets/opencode.json + ui/ + routes/
context: terminal user interface (TUI) — monochrome cell grid, ANSI/RGBA color, text attributes
colors:
  background: "#0a0a0a"
  backgroundPanel: "#141414"
  backgroundElement: "#1e1e1e"
  borderSubtle: "#3c3c3c"
  border: "#484848"
  borderActive: "#606060"
  text: "#eeeeee"
  textMuted: "#808080"
  primary: "#fab283"
  secondary: "#5c9cf5"
  accent: "#9d7cd8"
  error: "#e06c75"
  warning: "#f5a742"
  success: "#7fd88f"
  info: "#56b6c2"
  diffAdded: "#4fd6be"
  diffRemoved: "#c53b53"
  diffAddedBg: "#20303b"
  diffRemovedBg: "#37222c"
  lightBackground: "#ffffff"
  lightBackgroundPanel: "#fafafa"
  lightBackgroundElement: "#f5f5f5"
  lightText: "#1a1a1a"
  lightTextMuted: "#8a8a8a"
  lightPrimary: "#3b7dd8"
  lightSecondary: "#7b5bb6"
  lightAccent: "#d68c27"
---

# Design System: OpenCode TUI

Extracted from source: `packages/tui/src/theme/` (33 JSON themes + generator),
`ui/dialog.tsx`, `ui/toast.tsx`, `ui/spinner.ts`, `ui/border.ts`,
`opencode/src/cli/cmd/run/` (footer, scrollback, splash). Default theme: `opencode`.

## 1. Visual Theme & Atmosphere

A **quiet, cockpit-dense instrument panel** wrapped in a warm, editorial color voice.
The screen is 95% reading surface: an append-only transcript of markdown, code and
diffs. All interactivity is compressed into a 4-row footer at the bottom edge.
Nothing glows, nothing bounces — attention is directed by *color temperature*
(warm peach = the product, violet = structure, green/red = diff truth) and by
typographic weight, never by decoration.

Taste spectrum ratings: **Density 8** (cockpit), **Variance 3** (predictable,
column-symmetric — a reading tool, not a landing page), **Motion 5** (fluid but
restrained: one scanner animation, live text streaming, instant dialog swaps).

The dark palette is built on a **12-step neutral ramp** from `#0a0a0a` to `#eeeeee`
— near-black, never pure black. The single warm accent (`#fab283`, "Ember Peach")
is the brand's only saturated warm note; everything else is cool gray, soft violet,
or functional status color. The light theme inverts the voice: the brand note
becomes a deeper blue (`#3b7dd8`), and the warm role moves to amber (`#d68c27`).

## 2. Color Palette & Roles

### Primary Foundation (dark mode)
- **Void Black** `#0a0a0a` — root background (darkStep1). Never pure `#000000`.
- **Panel Charcoal** `#141414` — dialog/toast/sidebar surfaces (darkStep2).
- **Element Graphite** `#1e1e1e` — raised elements, menus, code-block fill (darkStep3).
- **Whisper Border** `#3c3c3c` — subtle separators (darkStep6).
- **Border Stone** `#484848` — default borders (darkStep7).
- **Border Lit** `#606060` — active/focused borders (darkStep8).

### Accent & Interactive
- **Ember Peach** `#fab283` — *the* brand primary: prompt glyph, links, list markers,
  function names, selected affordances. One warm note only.
- **Structure Violet** `#9d7cd8` — accent: markdown headings, keywords, agent badges.
- **Signal Blue** `#5c9cf5` — secondary: info-adjacent actions, secondary badges.

### Typography & Text Hierarchy
- **Paper White** `#eeeeee` — primary text (darkStep12).
- **Ash Gray** `#808080` — muted text: metadata, comments, timestamps (darkStep11).
- **Inverse Ink** — selected list item text reuses the *background* color
  (selection = filled chip of primary color with bg-colored glyphs).

### Functional States
- **Error Coral** `#e06c75` · **Warning Amber** `#f5a742` · **Success Mint** `#7fd88f` · **Info Cyan** `#56b6c2`
- **Diff Truth**: added `#4fd6be` on deep-teal wash `#20303b`; removed `#c53b53` on
  oxblood wash `#37222c`; hunk headers in periwinkle `#828bb8`.
- **Thinking Dim** — reasoning blocks render the full syntax palette at **60% alpha**
  (`thinkingOpacity: 0.6`) — a literal "fading thought" treatment.

### Light mode anchors
Canvas `#ffffff`, panel `#fafafa`, element `#f5f5f5`, ink `#1a1a1a`, muted `#8a8a8a`,
brand shifts to **Deep Blue** `#3b7dd8`, warm role to **Amber** `#d68c27`,
violet deepens to `#7b5bb6`.

## 3. Typography Rules

Terminal = fixed monospace cell grid. Hierarchy is carried by **weight, attribute
and color**, never by size:

- **Display/H1**: heading color (Structure Violet), **bold + underline**.
- **H2–H6**: heading color, bold (no underline). Same cell size — rank by weight only.
- **Body**: Paper White, default weight, word-wrapped, generous line spacing implied
  by blank spacer rows between transcript entries (1 blank row rhythm).
- **Strong**: Warning Amber + bold. **Emphasis**: Soft Yellow `#e5c07b` + italic.
- **Inline code**: Mint green on root background. **Code blocks**: Paper White on
  Element Graphite, tree-sitter palette (below).
- **Links**: Ember Peach underlined; link label text in Info Cyan underlined.
- **Quotes**: Soft Yellow italic. **HR/rule**: Ash Gray.
- **Syntax**: keyword = violet italic · function = peach · string = mint ·
  number = amber · type = yellow · operator = cyan · comment = ash italic ·
  variable = coral · builtins = error red.
- **Prompt glyph**: Structure Violet — the single affordance marking "you type here".

Banned (observed conventions): no emoji in chrome, no pure-black backgrounds,
no second warm accent, no proportional-font thinking — the grid is sacred.

## 4. Component Stylings

* **Prompt footer**: 4-row dock at the bottom edge, full width. Contains the input,
  a status line (agent name titlecased, model label, cwd with `~` abbreviation),
  and swaps wholesale for permission/question/palette views. The only reactive zone.
* **Transcript entries**: append-only rows; entries separated by one blank spacer
  row; streaming entries mutate in place until stable, then freeze forever.
* **Dialogs**: centered card (width 60 / 88 / 116 cols by size), Panel Charcoal fill,
  `paddingTop: 1`, floating over a full-screen scrim of black at **59% alpha**
  (`RGBA(0,0,0,150)`), vertically anchored ~¼ from the top. Esc or Ctrl-C pops the
  stack; focus returns to the previously focused element.
* **Toasts**: single-slot, top-right (`top:2, right:2`), max width 60, Panel Charcoal,
  padding `2×1`, **left+right vertical bars only** (`┃`) tinted by variant color,
  bold optional title, 5s default auto-dismiss. Never stacked, never centered.
* **Borders**: `SplitBorder` = side rails only — vertical `┃`, all other border
  glyphs empty. Boxes breathe through padding, not frames. Full boxes are rare.
* **Spinner ("scanner")**: 8-cell knight-rider sweep, diamond glyphs `⬥◆⬩⬪`,
  bidirectional with **hold frames at both ends** (30 start / 9 end), 6-step alpha
  trail (1.0 → 0.9 with 1.15× bloom → 0.65ⁿ decay), inactive cells at 20% alpha.
  Motion with weight and rest — never a naive rotating slash.
* **Splash**: ASCII wordmark + session meta written into scrollback at entry/exit —
  branding that scrolls away naturally instead of a modal loading screen.
* **Lists/palette**: selection = primary-filled row with background-colored text;
  fuzzy-ranked; muted glyphs for categories; keybind hints right-aligned in Ash Gray.

## 5. Layout Principles

- **Two-zone split (direct mode)**: scrollback (all rows − 4) + footer (4 rows).
  Scrollback is write-once; footer repaints. This *is* the layout system.
- **Full-screen column (rich mode)**: root flex column — route content (home or
  session) grows, sidebar docks right, footer/palette/dialogs overlay absolutely.
- **Reading measure**: transcript text wraps at terminal width with word wrap;
  dialogs cap at 116 cols; toasts cap at 60 — nothing reads wider than ~116 cells.
- **Rhythm**: single blank-row separators between transcript blocks; `paddingTop: 1`
  on cards; 2-cell horizontal padding on floating surfaces.
- **Resize behavior**: full re-layout on terminal resize; footer height is constant.
- **Touch analog**: mouse supported (click, scroll, drag-select); copy-on-select
  with toast confirmation; right-click copies. Selection suppresses dialog dismiss.

## 6. Motion & Interaction

- **One animation vocabulary**: the alpha-trail scanner. Thinking = palette at 60%
  alpha (a shimmer without movement). Streaming = text appearing. That's all.
- **Frame discipline**: 30fps cap in stream mode, 60fps in full mode; server events
  coalesced into 16ms batches → at most one repaint per frame.
- **Instant chrome**: dialogs/toasts/palette swap with zero transition — density
  beats decoration in a tool you drive all day.
- **Physics lives only in the scanner**: hold-at-ends + decay trail = the single
  "breathing" element that signals the machine is working.

## 7. Anti-Patterns (Banned)

- No pure black `#000000` (near-black ramp only) — except the 59%-alpha dialog scrim.
- No emoji or pictographic bullets in UI chrome; glyphs are geometric (`⬥◆┃·`).
- No neon glow / box-shadow simulation; no gradient fills behind text.
- No second warm accent competing with Ember Peach.
- No stacked toasts, no centered modals-for-everything, no spinner salad (one scanner).
- No re-rendering history — committed scrollback is immutable.
- No horizontal scroll in transcript — word wrap always.
- No fake precision (`99.99%`), no "Elevate/Seamless/Unleash" copy in strings.

## 8. Notes for Reimplementation (Rust/ratatui)

- Translate the 12-step ramp + 5 named accents into a `Theme` struct; the upstream
  JSON format (`defs` + `{dark, light}` variants + string refs) is directly
  serde-compatible — you can load OpenCode's own theme files unmodified.
- Recreate `SplitBorder` as a ratatui `BorderSet` with only `VERTICAL_LEFT/RIGHT`
  set to `┃`.
- Scrim = full-screen `Clear` + black block at ~60% alpha (ratatui: blend or a
  dimming pass over the underlying buffer).
- Scanner spinner: 8 cells, frame index → position with holds; per-cell alpha from
  the decay table in `ui/spinner.ts` (`deriveTrailColors`, `deriveInactiveColor`).
- Thinking blocks: same palette, `add_modifier(DIM)` or pre-multiplied alpha ≈ 0.6.

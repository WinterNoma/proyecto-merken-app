# PULSYS

Brand standard 01 · 14 November 2026 · Design proposal for team review

NASA Space Apps 2026 · Health Monitoring Software for Astronauts · Delivery: 15 November 2026

**Approved direction (14 November 2026): B / Basalt Standard + Flight Documentation.** Astra originally recommended A / Flight Graphite; the team chose Basalt. An institutional identity built around legible records, accountable language and quiet operational structure. This is an independent project identity; it does not imply NASA or ESA endorsement. No agency insignia, imitation agency logotypes or invented mission credentials.

The companion at `/brand.html` is a standalone brand specimen. All of its readings, identities and interactions are explicitly illustrative. It does not connect to the health core, save records, raise emergencies or synchronize data. This delivery does not modify application source or configuration.

## 1. Brand essence

### Mission

Make locally available health observations understandable to a crew member: what was recorded, what the core evaluated, what remains unknown and what action is available. Preserve that clarity when a connection to ground is unavailable.

### Personality

Institutional, technical, restrained and accountable. The visual reference is the organization of an agency technical manual or operational console: precise labels, reliable alignment and an explicit distinction between evidence and uncertainty. Authority comes from clarity, not ornament or dramatic language.

### Five design principles

1. **Evidence before interpretation.** Every reading has a unit, measurement time and provenance. Health assessments come from the core. A brand color never creates a medical conclusion.
2. **Make uncertainty legible.** No data, stale data, insufficient history and contextual data are different conditions. None is silently promoted to Stable.
3. **Reserve emphasis for consequence.** Red means Alarm or an urgent help action. Amber means Change. Neutral controls and decoration do not borrow these meanings.
4. **Support deliberate action.** Use explicit verbs, predictable placement, persistent alarm information and one clear confirmation for Need help.
5. **Remain useful offline.** Package fonts and assets locally. Distinguish a saved local record from a record synchronized with ground.

No neon, glow, animated scanning, decorative waveforms, star fields, glass panels, pictographic emoji or fictional system readouts. Low-light suitability is a design intention requiring device and cabin testing, not a verified physiological claim.

## 2. Voice and tone

### Rules

Use English throughout. Use sentence case for interface copy; uppercase is reserved for PULSYS, DEMO and short document identifiers. State the observation first, then its scope or limitation, then a useful action. Prefer one short sentence per idea. Avoid exclamation marks, humor, praise, reassurance without evidence and diagnostic labels such as “healthy,” “critical condition” or a disease name inferred by the interface.

Use **reading** for a measurement, **evaluation** for a core result and **request** for a help event. Use **Stable**, never “All clear.” Use **Change**, never “Danger” for `change`. Preserve a core message when appropriate; do not rewrite away uncertainty or escalation. This application does not provide medical advice. Do not author treatment instructions in the presentation layer.

### Ready-to-use copy

Curly-brace fields below must be supplied by the core or a formatted record; they are not defaults. Show the relevant `source_kind` next to the evaluation. A `demo_heuristic` evaluation must also show **“Demo rule, not medical advice.”**

| Condition | Heading or label | Supporting copy | Action / qualification |
| --- | --- | --- | --- |
| `stable` | Stable | No change detected in the evaluated history. | View history. This does not mean every indicator is evaluated. |
| `change` | Change detected | A change was detected in the recent readings. | Review readings. Append direction or “Persistent change” only when returned by the core. |
| `alarm` | Alarm | The core reported an alarm. Review the event details. | View event. Show the actual escalation separately. |
| `not_evaluated`, unknown reason | Not evaluated | An evaluation is not available for this indicator. | Preserve any recorded value and show its time. |
| `no_data` | No data | No reading is available for this indicator. | Record a reading. Never show 0 as a missing value. |
| `stale` | Stale reading | Last reading: {measured_at_utc}. It is too old for a current evaluation. | View history. Show the recorded value as “Last recorded,” never as current. |
| `too_few_data` | Insufficient history | Need {history_needed} previous readings ({history_count}/{history_needed}). | View history. The latest reading is not included in `history_count`. |
| `not_applicable` | Context only | Recorded for context; no health evaluation applies. | Show the value with a neutral information icon, not a health status. |
| Connection request failed | Connection unavailable | This request could not be completed. Local records remain available. | Try again. Do not claim that a failed save succeeded. |
| Local core unavailable | Local service unavailable | The local service could not be reached. This action was not confirmed. | Try again. Keep the user's entry. |
| Need help confirmation | Record an urgent help request? | This will save an urgent event on this device. Ground receipt is not confirmed. | Cancel / Record help request. Entry button: **Need help**. |
| Need help saved, `pending` | Help request saved locally | Synchronization pending. Ground receipt is not confirmed. | View event. No “Help is on the way.” |
| Need help save failed | Help request not saved | The request could not be saved. Try again. | Try again / Cancel. Keep the dialog open and preserve entered text. |
| `is_demo: true` | DEMO | Simulated records. Not uploaded to ground. | Keep DEMO visible on the selected profile, readings and event views. |
| First real record from DEMO | Demo data will be removed | Saving real data will remove the demo data. | Cancel / Save real data. Also applies to a real Need help event. |
| Pending synchronization | Synchronization pending | {pending_count} records pending. Saved locally; not yet synchronized. | View queue. Do not promise a transmission time. |
| A confirmed synchronized record | Synchronized | This record is marked as synchronized. | Do not infer that a person on ground has reviewed it. |

### State and escalation contract

`ui/src/vital-status.ts` currently exposes `ok / warn / alarm`. The new visual aliases are `ok → green`, `warn → amber`, `alarm → red`. This is a visual correspondence only: a reading-level `ok` does not prove a history-level `stable`. The future implementation must use the authoritative core result, not recalculate health states from this document.

| Core status | Display | Semantic token | Icon |
| --- | --- | --- | --- |
| `stable` | Stable | `--mk-ok` | Circle with check |
| `change` | Change | `--mk-warn` | Triangle with exclamation |
| `alarm` | Alarm | `--mk-alarm` | Octagon with exclamation |
| `not_evaluated` | Not evaluated + reason | `--mk-neutral` | Circle with short dash |
| `not_evaluated / no_data` | No data | `--mk-neutral` | Empty record rectangle with dash |
| `not_evaluated / stale` | Stale reading | `--mk-neutral` | Clock |
| `not_evaluated / too_few_data` | Insufficient history | `--mk-neutral` | Incomplete series of bars |
| `not_evaluated / not_applicable` | Context only | Neutral text, no health badge | Information circle |

Availability and assessment are separate. Keep an existing persistent alarm visible if the next reading is stale. Never demote an alarm because a connection is unavailable. The overall label follows `status.overall`; if some series are unevaluated, show the missing evaluations and `worst`, not a green aggregate fabricated from the visible cards.

| Escalation | Presentation rule |
| --- | --- |
| `continue` | No additional action banner. Do not say “Continue activity” as medical advice. |
| `self_care` | Label “Core guidance”; render only approved core-provided content and its source. Do not invent recommendations. |
| `flag_for_ground` | “Flagged for ground review”; show synchronization separately. Flagged does not mean received. |
| `urgent` | Expose the help flow prominently. Confirm before creating a `need_help` event; do not duplicate an already recorded event. |

`need_help` currently creates an urgent persistent alarm. There is no resolve endpoint. Do not add a dismiss, acknowledge-as-resolved or auto-expiry control. The contract currently leaves ground synchronization pending; the brand must not imply a working delivery service.

## 3. Wordmark

### Construction

**PULSYS**, uppercase, Barlow Semi Condensed Semibold (600), natural glyph proportions, 0.06em tracking. One word, one line, no symbol or tagline incorporated into the mark. The narrower face provides a distinct document-header signature without borrowing the NASA worm or ESA mark.

- Horizontal master: [`logo/pulsys-wordmark.svg`](logo/pulsys-wordmark.svg), pale ink `#E8EDF2`, transparent background; use on the dark palette.
- Monochrome master: [`logo/pulsys-wordmark-mono.svg`](logo/pulsys-wordmark-mono.svg), `currentColor`, default black on a light background. In inline SVG, set one contrasting ink through `color`; use a light ink when reversed.
- Clear space: let **H** be the visible capital-letter height. Keep at least **0.5H** on every side, measured from the visible glyph edges. Other UI labels, borders and imagery stay outside that area. The specimen's dashed guide is instructional and never part of the logo.
- Minimum digital width: **112 CSS px for the visible word**, excluding clear space. Use at least 24 mm in print. These are proposed minimums pending device and print checks.
- Keep the mark unrotated and uncondensed. Do not redraw letters, add gradients, insert status lights, outline the word or recolor individual letters.

The supplied SVGs use editable text, **not outlined paths**. The font and weight are recorded in each SVG. The live page renders the mark inline with the locally imported font. Standalone SVG viewers need Barlow Semi Condensed 600 installed; an SVG loaded as an image does not inherit page web fonts. For final external distribution, open with the specified font and convert text to paths, then verify spacing and clear space. Do not export a fallback-font rendering as the approved mark.

## 4. Color system

### Three complete dark palettes

All use opaque backgrounds, pale text and a restrained blue action color. Shared semantic colors and tints below complete each palette and **do not change meaning when a palette changes**. No full-screen alarm-red backgrounds.

| Token suffix (`--mk-`) | A · Flight Graphite · Recommended | B · Basalt Standard | C · Archive Navy |
| --- | --- | --- | --- |
| `bg` | `#10151B` | `#171717` | `#101827` |
| `surface` | `#192129` | `#222222` | `#1A2638` |
| `surface-raised` | `#222D37` | `#2D2D2D` | `#26354A` |
| `text` | `#E8EDF2` | `#EEECE8` | `#E8EDF5` |
| `text-muted` | `#B2BDC8` | `#C0BBB3` | `#B6C2D3` |
| `border` | `#81909F` | `#95918B` | `#8A9AB2` |
| `divider` | `#36424F` | `#47443F` | `#3A4C65` |
| `accent`, `focus` | `#A6C4E0` | `#C2CDE0` | `#B8C7E5` |
| `accent-hover` | `#BDD3E8` | `#D4DCEC` | `#CFD9ED` |
| `accent-pressed` | `#90B3D2` | `#ABBAD2` | `#A5B8DC` |
| `on-accent` | `#10151B` | `#171717` | `#101827` |

**A / Flight Graphite.** Recommended for its neutral operational surfaces, distinct steel-blue action ink and clear separation from the outgoing cyan HUD. It carries technical authority without making the entire interface blue.

**B / Basalt Standard.** Warm grayscale with a desaturated blue action ink. The most document-like option; useful if the team wants less association with a flight console. Its softer warmth is a design preference, not a demonstrated eye-strain benefit.

**C / Archive Navy.** An ink-blue foundation reminiscent of technical publication covers. Strongest institutional presence, but more visually colored across large surfaces; use sparingly in a dense monitoring view.

| Shared token | Foreground | Background / related token | Fixed meaning |
| --- | --- | --- | --- |
| `ok` / `ok-bg` | `#A5CCB3` | `#1C3028` | Evaluated stable; circle/check + text |
| `warn` / `warn-bg` | `#E8C27D` | `#352C1D` | Change; triangle + text |
| `alarm` / `alarm-bg` | `#F0A2A2` | `#382226` | Alarm; octagon + text |
| `neutral` / `neutral-bg` | `#B2BDC8` | `#252E38` | Unknown, absent, stale, history or context; specific icon + text |
| `emergency` | `#F0A2A2` | `on-emergency: #10151B` | Need help / urgent confirmation |
| `emergency-hover` | `#F5B7B7` | `on-emergency: #10151B` | Pointer hover |
| `emergency-pressed` | `#DF9090` | `on-emergency: #10151B` | Pressed |

Blue is interaction and focus, never a health assessment. Pending synchronization and DEMO are neutral operational labels with icons and explicit text. Do not use green to mean online or amber to mean offline. Decorative dividers are not sufficient control boundaries; use `border` for controls and `focus` for focus rings.

### Contrast validation

Calculated from the exact opaque sRGB hex values using WCAG relative luminance: normalize channels to 0–1; linearize with `c / 12.92` for `c <= 0.04045`, otherwise `((c + 0.055) / 1.055)^2.4`; luminance is `0.2126R + 0.7152G + 0.0722B`; ratio is `(Llighter + 0.05) / (Ldarker + 0.05)`. Displayed ratios are rounded to two decimals; pass/fail uses unrounded values.

The text target is **4.5:1**, including small secondary text. Essential graphics, boundaries and focus indicators target **3:1** against adjacent backgrounds. See [W3C text contrast](https://www.w3.org/WAI/WCAG22/Understanding/contrast-minimum.html) and [W3C non-text contrast](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html). These checks validate the listed pairs, not complete WCAG conformance.

| Pair | Flight Graphite | Basalt Standard | Archive Navy | Requirement |
| --- | ---: | ---: | ---: | --- |
| Text / surface | 13.81:1 | 13.48:1 | 12.96:1 | Text AA |
| Muted / surface | 8.53:1 | 8.33:1 | 8.45:1 | Text AA |
| Control border / raised | 4.29:1 | 4.39:1 | 4.35:1 | Non-text AA |
| Focus / raised | 7.74:1 | 8.59:1 | 7.30:1 | Non-text AA |
| On-accent / pressed (darkest action fill) | 8.35:1 | 9.12:1 | 8.87:1 | Text AA |
| Stable / raised | 7.93:1 | 7.80:1 | 7.03:1 | Text AA |
| Change / raised | 8.30:1 | 8.16:1 | 7.36:1 | Text AA |
| Alarm / raised | 6.94:1 | 6.82:1 | 6.15:1 | Text AA |
| Neutral / raised | 7.34:1 | 7.22:1 | 6.51:1 | Text AA |

The raised surface is the lightest general dark surface, so it is the limiting case for these pale borders and status foregrounds. Each palette's body text and muted text also pass on its background and raised surface; the live matrix calculates all three. Buttons keep opaque colors in hover, pressed, focus and disabled specimens.

| Shared tinted pair | Contrast | Result |
| --- | ---: | --- |
| Stable `#A5CCB3` / `#1C3028` | 7.91:1 | Text AA |
| Change `#E8C27D` / `#352C1D` | 8.14:1 | Text AA |
| Alarm `#F0A2A2` / `#382226` | 7.29:1 | Text AA |
| Neutral `#B2BDC8` / `#252E38` | 7.21:1 | Text AA |

Never reduce label opacity to suggest inactivity. Mark an unavailable action with a disabled control, explanatory text and a dashed border. Large white areas are limited to the monochrome print specimen. Actual low-light adaptation, anti-aliasing, WebView rendering and display brightness remain to be evaluated.

## 5. Typography

### A / Flight Documentation — recommended

**Headings: Barlow Semi Condensed 600. Body: Public Sans 400/600. Figures: Roboto Mono 400.**

Barlow's moderate compression gives headings a disciplined label-like character and differentiates PULSYS without a futuristic display face. Public Sans provides open, steady paragraph and control text. Roboto Mono separates measurements, record identifiers and timestamps from prose, with fixed-width digits. Keep condensed text out of long paragraphs. Three families introduce a clear functional hierarchy at the cost of more font files.

### B / Technical Record

**Headings: Archivo 600. Body: Source Sans 3 400/600. Figures: Source Code Pro 400.**

Archivo gives titles a wider, robust silhouette; Source Sans 3 supports a compact technical record with softer letter shapes; Source Code Pro gives tabular records an explicit instrument-data texture. This option is appropriate if broader titles and a denser reading rhythm are preferred. Check its smaller perceived body size on device before reducing any size tokens.

### C / Institutional Standard

**Headings: Libre Franklin 600. Body: Libre Franklin 400/600. Figures: Roboto Mono 400.**

One sans-serif family gives a consistent public-service document voice. The body and title share construction, relying on scale and weight for hierarchy. This is the quietest and smallest family set; it has less typographic distinction between navigation and content than A.

### Verified package inventory

Registry metadata and the published tar archives were inspected on 14 November 2026. Every listed `latin-{weight}.css` entry exists in the inspected version and each archive includes a LICENSE. All seven packages declare **OFL-1.1**, also identified by the linked Fontsource family pages. Authoring inspection used in-memory archives and did not install packages. Concurrent dependency changes subsequently made all imported font files available locally; those manifest changes are outside this delivery. Preserve each font's copyright and OFL notice when distributing it.

| Package / family evidence | Inspected version | Imported weights | Proposal |
| --- | --- | --- | --- |
| [`@fontsource/barlow-semi-condensed`](https://fontsource.org/fonts/barlow-semi-condensed) | 5.3.0 | 600 | A and fixed wordmark |
| [`@fontsource/public-sans`](https://fontsource.org/fonts/public-sans) | 5.3.0 | 400, 600 | A |
| [`@fontsource/roboto-mono`](https://fontsource.org/fonts/roboto-mono) | 5.3.0 | 400 | A, C |
| [`@fontsource/archivo`](https://fontsource.org/fonts/archivo) | 5.3.0 | 600 | B |
| [`@fontsource/source-sans-3`](https://fontsource.org/fonts/source-sans-3) | 5.3.1 | 400, 600 | B |
| [`@fontsource/source-code-pro`](https://fontsource.org/fonts/source-code-pro) | 5.3.1 | 400 | B |
| [`@fontsource/libre-franklin`](https://fontsource.org/fonts/libre-franklin) | 5.3.0 | 400, 600 | C |

Install from the repository root when ready:

```sh
npm install @fontsource/barlow-semi-condensed@5.3.0 @fontsource/public-sans@5.3.0 @fontsource/roboto-mono@5.3.0 @fontsource/archivo@5.3.0 @fontsource/source-sans-3@5.3.1 @fontsource/source-code-pro@5.3.1 @fontsource/libre-franklin@5.3.0
```

The preview imports only Latin upright 400 and/or 600, as listed. No italic, synthetic bold, remote CSS or runtime font CDN. All three options load for comparison; a later approved application implementation should retain only its chosen families. The fixed PULSYS wordmark does not switch when comparing body typography.

### Type scale and figures

| Role | Size | Weight / leading |
| --- | --- | --- |
| Document identifier | 12 px / 0.75rem | Body 600, 1.5; nonessential metadata only |
| Caption, timestamp, chip | 14 px / 0.875rem | Body 400/600 or data 400, 1.5 |
| Body and controls | 16 px / 1rem | Body 400/600, 1.6 |
| Component heading | 20 px / 1.25rem | Heading 600, 1.2 |
| Subsection | 24 px / 1.5rem | Heading 600, 1.2 |
| Section | 32 px / 2rem | Heading 600, 1.2 |
| Primary measurement | 48 px / 3rem | Data 400, 1.1 |
| Brandbook cover | 48–80 px, responsive | Heading 600, 1.1; never a monitoring label |

Use `font-variant-numeric: tabular-nums lining-nums` and `font-feature-settings: "tnum" 1, "lnum" 1` for measurements, timestamps and count columns. Right-align comparable numbers; keep a unit with its value, use separate aligned columns for mixed units, and preserve core precision. Do not pad invented decimal places. Use `72 bpm`, `36.7 °C` or `118/76 mmHg` only as clearly identified illustrative values; these examples define no thresholds or medical ranges. Use an em dash with the visible text “No data” when a value is absent.

Display UTC explicitly, for example `14 Nov 2026, 14:32 UTC`; use a machine-readable ISO timestamp in the `<time>` element. Do not confuse measurement time with synchronization time. Turn off font synthesis; a fallback rendering is a loading state, not the approved visual specimen.

## 6. Tokens and components

### Shared construction rules

- **Spacing:** 4 px base; 4, 8, 12, 16, 24, 32, 48, 64, 96 px. Card padding 24 px desktop / 16 px mobile. Component gaps 16–24 px. Reading lines generally stop at 65 characters.
- **Radii:** 0 for rules, 2 px for labels, 4 px for controls and cards, 8 px for dialogs. Avoid capsule buttons and decorative clipped corners.
- **Borders:** 1 px opaque control border; 3 px semantic leading rule for notices; 2 px focus outline with 3 px offset. Decorative structural rules use `divider`.
- **Elevation:** none on cards or hover. Dialog alone uses `0 16px 48px rgb(0 0 0 / 0.35)` plus an opaque surface and dark scrim. No colored shadow.
- **Motion:** color and border transitions only, 120 ms; optional disclosure opacity 180 ms. No automatic pan, pulse, blinking, number rolling or waveform animation. `prefers-reduced-motion: reduce` sets transition durations to zero and removes animation; status remains fully visible.
- **Targets:** at least 44 × 44 CSS px for controls; 8 px between competing actions. Keyboard focus is visible and never conveyed by color alone. Keep focus outlines clear of clipping containers.
- **Iconography:** original inline SVG geometry, 24 × 24 grid, 1.75 px stroke, round caps/joins, normally displayed at 20 px. A shape and a text label always accompany semantic color. No icon library is required or proposed; there is no additional icon dependency or third-party license. Repeated decorative icons use `aria-hidden="true"` and `focusable="false"`.

### Vital-sign card

Anatomy: indicator name → icon and status label → value with unit → reason → measurement time → source → optional honest history plot → explicit history action. Value uses data type; labels use body type. Keep a status label readable when the value is missing.

States: Stable uses circle/check; Change uses triangle and core-supplied direction/persistence; Alarm uses octagon and links to the event; Not evaluated preserves available values and gives the reason; No data shows an em dash, no trace and an entry action; Stale labels the value “Last recorded,” retains its original time and never implies current monitoring. Insufficient history shows counts from the core. Context-only indicators show no health badge. The preview omits numeric readings in Change/Alarm specimens rather than invent clinical trigger values.

### Status label

Anatomy: fixed-size icon + explicit short label; optional adjacent reason. Use a 1 px semantic outline, tinted opaque fill, 2 px radius and 8 px horizontal padding. This is information, not a button. Stable, Change, Alarm, Not evaluated, No data and Stale must remain distinguishable in grayscale by shape and wording. Do not convey change direction with color alone; include “Increasing” or “Decreasing” when known.

### Alarm banner and related notices

Anatomy: octagon + Alarm heading, affected indicator or event, core message, time/source, escalation, sync state and a View event action. Persistent until the core says otherwise; no close button for `need_help`. Keep it above the relevant data without covering navigation.

Use an alarm banner only for Alarm. The same layout becomes a **notice** for Change or availability conditions: triangle + Change, clock + Stale, record icon + No data, or neutral Not evaluated. Stable normally needs only a label; its notice in the specimen demonstrates anatomy. Static specimen banners are not live alerts. In the future app, announce a newly arrived urgent alarm once with an appropriate live region; avoid re-announcing it on every refresh.

### Buttons: primary, secondary, emergency

Primary: blue fill / dark text, specific action such as “Record a reading.” Secondary: surface fill, visible neutral border, text such as “View history.” Emergency: red fill / dark text, octagon + **Need help**. Keep emergency physically separated from routine actions. Never use “OK” as a confirmation action.

Interaction states for all three: default, hover, pressed, keyboard focus, busy and disabled. Hover/pressed use defined opaque fills, no scale or glow. Focus uses a 2 px offset ring. Busy retains a stable width, sets `aria-busy="true"`, blocks duplicate activation and says “Saving…” or “Recording…”. Disabled has a dashed boundary and a visible reason; no low-opacity text. Health status does not disable Need help, including No data and Stale. Offline operation alone does not disable local actions. The static busy/disabled specimens are intentionally inoperable and labeled as examples.

### Crew selector

Anatomy: visible “Crew member” label → native select with name and identifier → selected identity summary → separate evaluation label and DEMO provenance. No avatar conveys health or selection by itself. Use explicit selected text, not just a border. Names wrap in the summary; long names must not widen the page.

States: available/selected; loading (disabled select and “Loading crew…”); empty (disabled select and “No crew available”); load error (persistent message and Retry); and selected crew with each of the six evaluation/availability states. Use native keyboard behavior. Changing selection must clear or explicitly label previous readings until the new subject's data arrives. The specimen only changes a fictional identity and never loads real data.

### Sparkline

Anatomy: indicator + period → actual sample path with gaps → labeled plotted extent and unit → start/end time → equivalent text and a data table. Two-pixel line; no area glow, smoothing that invents extrema or animated ECG from a single pulse value. The plotted extent is a chart scale, not a medical reference range. Do not hide scale changes.

Stable/Change/Alarm: inherit the returned semantic state, but plot only the associated record series. Not evaluated: plot available points without implying a successful evaluation; one point is a dot, not a line. No data: no trace. Stale: preserve a historical trace, label it historical and end it at the last actual sample; do not extrapolate to now. Gaps remain gaps. In the brand specimen, Stable and Stale share six explicitly invented normal illustrative readings; Change and Alarm intentionally have no numeric series. No clinical thresholds are supplied.

### Confirmation dialog

Anatomy: specific question → selected crew identity → consequence → local/ground status → optional DEMO-removal consequence → Cancel and explicit confirmation. Use native `<dialog>` / `showModal()`, an accessible name and description, initial focus on Cancel, Escape to cancel, contained tab order and return focus to the opener. Keep content scrollable on short viewports. No backdrop-click confirmation or hold gesture.

States: closed; open for any selected health/data state; saving (block duplicate submission); local success with pending sync; failure with retry. A health status changes context, not the confirmation's meaning. Only show success after a core response. The preview instead says “Preview only” and never calls the core. Its state control demonstrates confirmation, saving, failure and local-result copy without inventing a backend result.

### Empty and unavailable data

Anatomy: neutral icon → specific heading → short reason → useful available action. Never insert placeholder telemetry to fill space. No data invites entry; insufficient history explains counts; stale points to history; unknown evaluation states say Not evaluated; context-only data stays informational. Empty history may coexist with a known current evaluation, so it must not erase a current Stable, Change or Alarm label. An alarm does not turn an empty panel into red telemetry. Loading is separate from absent data and uses text, not an endless spinner.

### Responsive and accessibility behavior

At 375 px, use one-column cards, wrapped controls, 16 px page gutters, full-width dialog actions and no fixed-width tables. Desktop uses a bounded reading width and two/three-column specimen grids. Layout tracks use `minmax(0, 1fr)`; long identifiers wrap. Do not hide horizontal overflow to conceal layout defects. Use semantic headings, labels, buttons and tables. Preserve browser zoom. Focus returns after dialog closure, buttons expose pressed state, and preview-only updates use a single polite status message. Reduced motion never hides an alarm.

## 7. Delivery and review boundary

Created files:

- `docs/brand/brandbook.md`
- `docs/brand/tokens.css`
- `docs/brand/logo/pulsys-wordmark.svg`
- `docs/brand/logo/pulsys-wordmark-mono.svg`
- `ui/brand.html`
- `ui/src/brand.ts`
- `ui/src/brand.css`

The preview imports the shared tokens from `../../docs/brand/tokens.css`. It has no API, remote asset, storage or Tauri dependency. Source links in this document are research references, not runtime dependencies. The existing Vite root is `ui`, so the intended development URL is `/brand.html`. The existing production build still targets the application entry; adding a brandbook production entry is outside this delivery and would require a later configuration change.

Verified during authorship: exact npm package metadata; all imported CSS paths in the published archives; programmatic color-pair calculations; source review against the core command contract; allowed-file scope. Not executed: dependency installation, `npx tsc --noEmit`, Vite build, browser rendering, keyboard/screen-reader tests, offline network test, physical low-light testing or SVG-to-path export. These remain review tasks after the team installs the packages. No claim of clinical validation or complete accessibility conformance is made.

Product references: [`ui/index.html`](../../ui/index.html), [`ui/src/styles.css`](../../ui/src/styles.css), [`ui/src/vital-status.ts`](../../ui/src/vital-status.ts), [`docs/api/comandos-core.md`](../api/comandos-core.md), [`docs/front/investigacion-front.md`](../front/investigacion-front.md). The earlier research informed the constraints; its proposed palette and typography were not adopted.

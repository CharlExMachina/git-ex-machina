# Design changes

Changes made to `mockup-appshell.html` after it was handed over, each at the design owner's request. `design/system/` is re-extracted from the mockup after every change.

## 2026-11-14: button texture

Feedback: light-mode buttons lacked dark mode's "premium" feel, and the dark primary button lacked the grey buttons' texture.

- `--shadow-primary` (new, both themes): the primary button gets a lit top edge, a shaded bottom edge, an edge ring and a drop. The grey buttons' 7% white highlight vanished on amber.
- `--shadow-badge` (new): a badge on the primary button is set into it.
- Light `--shadow-raised`: a firmer hairline (13% → 15%) and a short soft second drop.
- `--control-bottom` (new): buttons fill with `linear-gradient(var(--control), var(--control-bottom))`, white to paper in light. Dark keeps a flat fill.
- Light `--accent-top`: `#a46c20` → `#b07628`, so the primary gradient reads as a bevel.

## 2026-11-14: a sans inspector title

Feedback: the serif title was a sudden change of voice in an all-sans interface.

- `.display-sm` (the inspector's title, its only use): Instrument Serif 400 22px/26px → Hanken Grotesk 600 18px/24px, -0.01em. Chosen from three sans versions tried in the mockup (600 18/24, 500 20/26, 600 16/22).

---
icon: lucide/puzzle
hide:
  - toc
---

# Jigsaw Puzzle Generator

Generate an M×N grid of interlocking jigsaw pieces. Scramble and snap them back together on a free canvas, either manually or with a built-in solver. Optionally, export the pieces as JSON for a separate solver.

See [how the app works](how-it-works.md) for the piece model, snapping, and solver, or the [repo](https://github.com/vesuvisian/jigsaw) to run it locally.

## How to use

1. Set **Rows (M)** and **Cols (N)**, then click **Generate** for a fully assembled puzzle.
2. Click **Scramble** to separate pieces, scatter them on the canvas, and give each a random rotation.
3. **Drag** a piece to move its whole connected group, or drag empty canvas to box-select several groups and move or rotate them together. Matching edges within snap range lock together.
4. Tap or click to select a group. **Rotate**, **R**, right-click, double-tap, or a long-press turns the selection 90° clockwise. Arrow keys nudge. Click empty canvas or **Esc** clears the highlight.
5. **Solve Step** joins one matching pair. **Export…** downloads the current pieces as JSON.

<div class="jigsaw-frame-wrap" markdown="0">
  <iframe
    class="jigsaw-frame"
    src="app/"
    title="Jigsaw Puzzle Generator"
    allow="fullscreen"
  ></iframe>
</div>

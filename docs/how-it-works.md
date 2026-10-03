---
icon: lucide/waypoints
---

# How it works

The generator is one [egui](https://www.egui.rs/) UI that compiles both as a desktop app and to WebAssembly. Play state is split in two:

- A **puzzle** — each piece’s identity and current edge values
- A **board** — where those pieces sit on a free canvas, which groups they belong to, and paint order

Matching is numeric, not geometric. If two facing sides are complementary integers, they lock; the drawn tab is a pure function of that integer, so mates nest when they snap.

## Piece model

Each piece has:

| Field | Meaning |
| --- | --- |
| `id` | Random UUID assigned at generation. It never encodes grid position, so sorting by id does not recover the solution. |
| `rotation` | Quarter-turns clockwise from generation orientation (`0…3`). |
| `sides` | `[top, right, bottom, left]` as signed 128-bit integers. |

```
      top
left  [ ]  right
     bottom
```

Side values follow three rules:

- **`0`** — outer boundary of the puzzle (drawn flat)
- **Non-zero** — interior seam. The neighbor on that seam holds the same magnitude with opposite sign (`v` / `-v`)
- **Sign** — `+` is an outie (tab pointing outward); `-` is an innie (matching socket)

The magnitude is unique across the whole puzzle and **fully determines the interlocking silhouette**. Every byte of `|v|` seeds the spline, so shape is a function of the integer alone. Which side of a new seam is positive is chosen at random when the puzzle is generated.

A 90° clockwise turn cycles the array:

`[top, right, bottom, left]` → `[left, top, right, bottom]`

and increments `rotation` modulo 4. The integers themselves never change, only which compass slot they occupy.

## Generation

**Generate** builds an M×N grid already assembled.

1. Allocate `M × N` pieces with fresh UUIDs, `rotation = 0`, and all sides `0`.
2. Walk every **horizontal** seam: piece `(r, c)`’s right edge gets a unique signed `v`; the piece to its right gets `-v` on the left.
3. Walk every **vertical** seam: bottom of `(r, c)` gets another unique `v`; the piece below gets `-v` on the top.

Border sides stay `0`. The result is a solved lattice: complementary values already face each other, and every interior pairing id is used exactly twice.

## Tab outlines

Drawing never stores a path. For each side, `generate_edge_beziers` maps the integer onto a unit edge `(0, 0) → (1, 0)`:

- `0` is a single flat cubic
- Non-zero values use a four-segment classical die-cut: shoulder, neck, round bulb, and the mirrored far side
- Parameters (center, height, neck/head width, sway, tilt) come from a deterministic mix of `|v|`
- Interior joints are forced C¹ so the outline does not kink
- Sign flips `y`; a mate pair `v` / `-v` is an exact reflection across the seam

Those unit curves are then placed on the four sides of a cell (bottom and left are reversed so the walk stays clockwise), sampled, and filled with earcut triangulation. Tab height is about 26% of the cell, which is also the padding used for hit-testing and scramble visibility.

## Board

Pieces do not live in a grid at play time. Each has a **pose**:

- `pos` — top-left of its cell, in canvas units (cell size is 120)
- `group` — shared id for pieces that have locked together
- `z` — draw / hit-test order (last interacted group is on top)

**Generate** places everyone flush in row-major order as a single group. The play area is at least about 1.5× the assembled puzzle so there is room to scatter.

Hit-testing uses the cell rectangle plus tab overhang; the topmost overlapping piece wins. Dragging, rotating, and nudging always move the **whole group**.

## Scramble

**Scramble** does three things per piece:

1. Splits it into its own group
2. Places it at a random position that keeps the full outline (including outies) inside the play area
3. Cycles its sides a random 0–3 times, keeping `rotation` in sync

Ids are unchanged. After scramble, complementary values generally do not face each other until you rotate pieces back.

## Snap

After a drag, nudge, or rotate, the moved group looks for a neighbor it can join.

Two sides match when both are non-zero and `a == -b`. For each complementary pair of facing slots (right↔left, bottom↔top, …), the board computes the translation that would sit the two cells flush. If the nearest such translation is within **24** canvas units, the whole group slides by that delta and merges into the other group.

Only complementary **values** matter, not original grid neighbors. A piece will not snap to a non-mate even if the outlines look close, and it will not snap if the matching integers are not facing.

## Interaction

| Action | Effect |
| --- | --- |
| Drag | Moves the group; snap runs on release |
| Click | Selects the group (highlight follows the group, not a single cell) |
| **R** or right-click | Rotates the group 90° clockwise around its bounding-box center, then tries to snap |
| Arrow keys | Nudge, then snap |
| **Esc** | Clears selection |
| **Solve Step** | Joins one matching pair automatically |
| **Export…** | Writes the current pieces as JSON |
| **Show labels** | Overlays UUID stubs and the four side integers |

Rotation updates both poses (screen-space CW: `(x, y) → (-y, x)`) and each member’s `sides` / `rotation`.

## Automatic solver

**Solve Step** is a greedy, edge-first connector — not a search over the full layout.

1. Index every non-zero side value to its piece and compass slot.
2. Among complementary pairs that are still in **different groups**, pick the best by tier, then by piece index:
    - Both pieces have a flat (`0`) outer edge
    - Exactly one is an edge piece
    - Both are interior
3. Keep the **larger** group fixed (ties keep the lower-index piece’s group).
4. Rotate the smaller group until the mover’s complementary value faces the anchor.
5. Translate flush, merge groups, and bring the result to the front.

Repeat until one group remains. Valid generated puzzles always have a remaining cross-group seam until then.

## Export

**Export…** snapshots the puzzle in **generation (row-major) order**, with whatever orientations the pieces have **right now**. Positions and groups are not written; a downstream solver is expected to rematch on side values (and can rebuild the same outlines from those integers).

```json
{
  "pieces": [
    {
      "id": "3f9a2c1e-8b4d-4e6f-9a1b-2c3d4e5f6789",
      "rotation": 0,
      "sides": [0, -1834, 9912, 0]
    }
  ]
}
```

`rows` and `cols` are omitted unless **Include rows/cols** is checked. Native builds use a save dialog; the web build downloads the file instead.

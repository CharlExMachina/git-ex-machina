//! One row of the commit graph: lanes 14 apart,
//! 12 in from the edge, rails stroked 2 with round caps, a 6.5 ring of background around each
//! node, a 4.5 node (3.25 for a merge), and HEAD as a ring with a centre dot.

use gxm_core::graph::{Edge, GraphRow, Lit};
use rune::{Color, El, LineCap, Path, Shape, canvas};

use crate::theme::{Theme, size};

const PITCH: f32 = 14.0;
const PAD: f32 = 12.0;
const H: f32 = size::ROW_COMMIT;
const MID: f32 = H / 2.0;
const STROKE: f32 = 2.0;

/// The graph's lane palette: the order lanes take the `--lane-n` colours in, so the first branch
/// is amber, the second teal and the third rose.
const ORDER: [usize; 6] = [1, 2, 4, 3, 5, 6];

/// The colour of graph colour index `c`.
pub fn color(t: &Theme, c: usize) -> Color {
    t.lane(ORDER[c % ORDER.len()])
}

/// The graph column's width for `cols` lanes (`PAD * 2 + (cols - 1) * PITCH`).
pub fn width(cols: usize) -> f32 {
    PAD * 2.0 + (cols.max(1) - 1) as f32 * PITCH
}

fn x(col: usize) -> f32 {
    PAD + col as f32 * PITCH
}

/// How a row's node is drawn.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Node {
    Commit,
    Merge,
    Head,
}

pub struct RowLook<'a> {
    pub node: Node,
    /// The row sits under the uncommitted-changes row, whose dashed rail ends above it, so the
    /// node has no rail coming in from above.
    pub under_worktree: bool,
    /// The commit is selected: its node gets a ring in its lane's colour.
    pub selected: bool,
    /// With a pull request open, which parts of the row are its; the rest dims.
    pub light: Option<&'a Lit>,
}

/// How far what isn't the open pull request's dims, as an opacity.
pub const DIM: f32 = 0.38;

/// A dimmed line's colour: solid, as the lane at [`DIM`] over the canvas the graph sits on.
/// A translucent stroke would double up wherever two pieces overlap (the round caps where rows
/// meet, a curve's segments) and bead the line.
fn dimmed(t: &Theme, c: Color) -> Color {
    crate::theme::mix(c, DIM, t.canvas)
}

/// Strokes the cubic `[p0, c1, c2, p3]` in short pieces, its colour easing from `start` to
/// `end` along it (Rune strokes in one colour). The colours are solid, so the pieces' round caps
/// overlap without a seam.
fn fading(mut shape: Shape, [p0, c1, c2, p3]: [[f32; 2]; 4], start: Color, end: Color) -> Shape {
    const PIECES: usize = 16;
    let at = |u: f32| {
        let v = 1.0 - u;
        let (a, b, c, d) = (v * v * v, 3.0 * v * v * u, 3.0 * v * u * u, u * u * u);
        [a * p0[0] + b * c1[0] + c * c2[0] + d * p3[0], a * p0[1] + b * c1[1] + c * c2[1] + d * p3[1]]
    };
    for i in 0..PIECES {
        let (u0, u1) = (i as f32 / PIECES as f32, (i + 1) as f32 / PIECES as f32);
        let mid = (u0 + u1) / 2.0;
        let ease = mid * mid * (3.0 - 2.0 * mid);
        shape = shape.line(at(u0), at(u1), STROKE, LineCap::Round, crate::theme::mix(end, ease, start));
    }
    shape
}

/// The selected node's ring, and the cut-out of background under it that stops the rails a
/// little outside the ring.
const SELECTED_RING: f32 = 8.0;
const SELECTED_CUT: f32 = 10.25;

pub fn row(t: &Theme, g: &GraphRow, cols: usize, look: &RowLook<'_>) -> El {
    let nx = x(g.lane);
    let lit = |k: usize| look.light.is_none_or(|l| l.edges.get(k).copied().unwrap_or(false));
    let ink_of = |c: Color, on: bool| if on { c } else { dimmed(t, c) };
    // A lane running dimmed through the row, as its colour: a lit curve meeting it turns into it.
    let dimmed_lane = |lane: usize| {
        g.edges.iter().enumerate().find_map(|(k, e)| match *e {
            Edge::Pass { lane: l, color: c } if l == lane && !lit(k) => Some(dimmed(t, color(t, c))),
            _ => None,
        })
    };
    let mut shape = Shape::new();
    let (mut up, mut down) = (None, None);
    // What dims is drawn first, so the pull request's lines run over it.
    let mut order: Vec<usize> = (0..g.edges.len()).collect();
    order.sort_by_key(|&k| lit(k));
    for k in order {
        let on = lit(k);
        match g.edges[k] {
            Edge::Pass { lane, color: c } => {
                shape = shape.line([x(lane), 0.0], [x(lane), H], STROKE, LineCap::Round, ink_of(color(t, c), on));
            }
            Edge::In { from, .. } if from == g.lane => up = Some(on || up == Some(true)),
            Edge::In { from, color: c } => {
                let xc = x(from);
                let curve = [[xc, 0.0], [xc, 10.0], [nx, 6.0], [nx, MID]];
                // Out of a dimmed lane into the pull request, the line turns from that lane's
                // colour into its own.
                if let Some(base) = dimmed_lane(from).filter(|_| on) {
                    shape = fading(shape, curve, base, color(t, c));
                } else {
                    let p = Path::new().move_to(curve[0]).cubic_to(curve[1], curve[2], curve[3]);
                    shape = shape.stroke_path(&p, STROKE, LineCap::Round, ink_of(color(t, c), on));
                }
            }
            Edge::Out { to, .. } if to == g.lane => down = Some(on),
            Edge::Out { to, color: c } => {
                let xc = x(to);
                let curve = [[nx, MID], [nx, 24.0], [xc, 20.0], [xc, H]];
                // From the pull request into a dimmed lane (its base), the line turns into that
                // lane's colour as it goes.
                if let Some(base) = dimmed_lane(to).filter(|_| on) {
                    shape = fading(shape, curve, color(t, c), base);
                } else {
                    let p = Path::new().move_to(curve[0]).cubic_to(curve[1], curve[2], curve[3]);
                    shape = shape.stroke_path(&p, STROKE, LineCap::Round, ink_of(color(t, c), on));
                }
            }
        }
    }
    let lane = color(t, g.color);
    let node_on = look.light.is_none_or(|l| l.node);
    if look.under_worktree {
        // The worktree's dashed rail comes down into this row and ends under the node's ring.
        shape = dashed_rail(shape, nx, 0.0, MID, H, ink_of(lane, look.light.is_none()));
    } else if let Some(on) = up {
        shape = shape.line([nx, 0.0], [nx, MID], STROKE, LineCap::Round, ink_of(lane, on));
    }
    if let Some(on) = down {
        shape = shape.line([nx, MID], [nx, H], STROKE, LineCap::Round, ink_of(lane, on));
    }
    // The cut-out is the background itself, so it hides dimmed lines as cleanly as lit ones.
    let ink = ink_of(lane, node_on);
    if look.selected {
        shape = shape.fill(circle([nx, MID], SELECTED_CUT), t.canvas).stroke_with(
            circle([nx, MID], SELECTED_RING),
            true,
            1.5,
            LineCap::Round,
            ink,
        );
    } else {
        shape = shape.fill(circle([nx, MID], 6.5), t.canvas);
    }
    shape = match look.node {
        Node::Head => shape
            .stroke_with(circle([nx, MID], 4.5), true, STROKE, LineCap::Round, ink)
            .fill(circle([nx, MID], 1.75), ink),
        Node::Merge => shape.fill(circle([nx, MID], 3.25), ink),
        Node::Commit => shape.fill(circle([nx, MID], 4.5), ink),
    };
    canvas(shape).w(width(cols)).h(H)
}

/// The lanes running past a row inserted above row `g` (the pull request's row): every line
/// that crosses `g`'s top edge, straight through, dimmed unless the pull request's.
pub fn passing(t: &Theme, g: &GraphRow, cols: usize, light: Option<&Lit>, h: f32) -> El {
    let mut shape = Shape::new();
    for (k, e) in g.edges.iter().enumerate() {
        let (lane, c) = match *e {
            Edge::Pass { lane, color } => (lane, color),
            Edge::In { from, color } => (from, color),
            Edge::Out { .. } => continue,
        };
        let on = light.is_none_or(|l| l.edges.get(k).copied().unwrap_or(false));
        let ink = if on { color(t, c) } else { dimmed(t, color(t, c)) };
        shape = shape.line([x(lane), 0.0], [x(lane), h], STROKE, LineCap::Butt, ink);
    }
    canvas(shape).w(width(cols)).h(h)
}

/// The uncommitted row's mark: a ring of six even dashes, stroked like the graph's rails
/// (gaps at the top and bottom, where the rail leaves it), and a dashed rail in `lane` running
/// down into HEAD's row, where [`row`] continues it under HEAD's node. Before the first commit
/// there's no HEAD, and no rail.
pub fn worktree(t: &Theme, staged: bool, has_head: bool, lane: Color, cols: usize) -> El {
    let c = [PAD, MID];
    let mut shape = Shape::new();
    if staged {
        shape =
            shape.fill(circle(c, RING_R), t.accent).stroke_with(circle(c, RING_R), true, 1.5, LineCap::Butt, t.accent);
    } else {
        for arc in dashed_ring(c, RING_R, 6) {
            shape = shape.stroke_path(&arc, STROKE, LineCap::Round, lane);
        }
    }
    if has_head {
        shape = dashed_rail(shape, PAD, RAIL_START, H, 0.0, lane);
    }
    canvas(shape).w(width(cols)).h(H)
}

/// The worktree ring's radius.
const RING_R: f32 = 4.5;
/// Where the dashed rail starts, in the worktree row: just under the ring.
const RAIL_START: f32 = MID + RING_R + 2.0;
/// The rail's dashes: `DASH` long (plus round caps of half the stroke at each end), every
/// `DASH + GAP`, measured from `RAIL_START` across both rows so they meet at the row edge.
const DASH: f32 = 2.5;
const GAP: f32 = 4.0;

/// Dashes of the worktree rail between `from` and `to` in a row whose top is `row_top` points
/// below the worktree row's top.
fn dashed_rail(mut shape: Shape, x: f32, from: f32, to: f32, row_top: f32, color: Color) -> Shape {
    let period = DASH + GAP;
    let mut s = RAIL_START;
    while s < row_top + to {
        let (a, b) = ((s - row_top).max(from), (s + DASH - row_top).min(to));
        if b > a {
            shape = shape.line([x, a], [x, b], STROKE, LineCap::Round, color);
        }
        s += period;
    }
    shape
}

/// A circle as a closed polygon, fine enough to stay round at twice the size.
fn circle(c: [f32; 2], r: f32) -> Vec<[f32; 2]> {
    const N: usize = 48;
    (0..N)
        .map(|i| {
            let a = i as f32 / N as f32 * std::f32::consts::TAU;
            [c[0] + r * a.cos(), c[1] + r * a.sin()]
        })
        .collect()
}

/// `n` even dashes round a circle, each a smooth arc (one cubic Bézier) with round caps, placed
/// so the top and the bottom fall in gaps. The caps lengthen each dash by the stroke's width,
/// so the arcs themselves cover only 30% of each step.
fn dashed_ring(c: [f32; 2], r: f32, n: usize) -> Vec<Path> {
    let step = std::f32::consts::TAU / n as f32;
    let half = step * 0.3 / 2.0;
    let at = |a: f32| [c[0] + r * a.cos(), c[1] + r * a.sin()];
    (0..n)
        .map(|k| {
            // Dashes centred on 0°, 60°, 120°…: the bottom (90°) and top (270°) fall between.
            let mid = step * k as f32;
            let (a0, a1) = (mid - half, mid + half);
            // The standard cubic for a circular arc: handles tangent to the circle, k·r long.
            let k = 4.0 / 3.0 * ((a1 - a0) / 4.0).tan() * r;
            let (p0, p3) = (at(a0), at(a1));
            let p1 = [p0[0] - k * a0.sin(), p0[1] + k * a0.cos()];
            let p2 = [p3[0] + k * a1.sin(), p3[1] - k * a1.cos()];
            Path::new().move_to(p0).cubic_to(p1, p2, p3)
        })
        .collect()
}

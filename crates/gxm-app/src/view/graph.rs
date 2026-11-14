//! One row of the commit graph: lanes 14 apart,
//! 12 in from the edge, rails stroked 2 with round caps, a 6.5 ring of background around each
//! node, a 4.5 node (3.25 for a merge), and HEAD as a ring with a centre dot.

use gxm_core::graph::{Edge, GraphRow};
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

pub struct RowLook {
    pub node: Node,
    /// The row sits under the uncommitted-changes row, whose dashed rail ends above it, so the
    /// node has no rail coming in from above.
    pub under_worktree: bool,
    /// The background the node's ring is cut from (the selected row's raised fill).
    pub ground: Color,
}

pub fn row(t: &Theme, g: &GraphRow, cols: usize, look: &RowLook) -> El {
    let nx = x(g.lane);
    let mut shape = Shape::new();
    let mut ups = Vec::new();
    let mut downs = Vec::new();
    for e in &g.edges {
        match *e {
            Edge::Pass { lane, color: c } => {
                shape = shape.line([x(lane), 0.0], [x(lane), H], STROKE, LineCap::Round, color(t, c));
            }
            Edge::In { from, .. } if from == g.lane => ups.push(()),
            Edge::In { from, color: c } => {
                let xc = x(from);
                let p = Path::new().move_to([xc, 0.0]).cubic_to([xc, 10.0], [nx, 6.0], [nx, MID]);
                shape = shape.stroke_path(&p, STROKE, LineCap::Round, color(t, c));
            }
            Edge::Out { to, .. } if to == g.lane => downs.push(()),
            Edge::Out { to, color: c } => {
                let xc = x(to);
                let p = Path::new().move_to([nx, MID]).cubic_to([nx, 24.0], [xc, 20.0], [xc, H]);
                shape = shape.stroke_path(&p, STROKE, LineCap::Round, color(t, c));
            }
        }
    }
    let ink = color(t, g.color);
    if look.under_worktree {
        // The worktree's dashed rail comes down into this row and ends under the node's ring.
        shape = dashed_rail(shape, nx, 0.0, MID, H, ink);
    } else if !ups.is_empty() {
        shape = shape.line([nx, 0.0], [nx, MID], STROKE, LineCap::Round, ink);
    }
    if !downs.is_empty() {
        shape = shape.line([nx, MID], [nx, H], STROKE, LineCap::Round, ink);
    }
    shape = shape.fill(circle([nx, MID], 6.5), look.ground);
    shape = match look.node {
        Node::Head => shape
            .stroke_with(circle([nx, MID], 4.5), true, STROKE, LineCap::Round, ink)
            .fill(circle([nx, MID], 1.75), ink),
        Node::Merge => shape.fill(circle([nx, MID], 3.25), ink),
        Node::Commit => shape.fill(circle([nx, MID], 4.5), ink),
    };
    canvas(shape).w(width(cols)).h(H)
}

/// The uncommitted row's mark: a ring of six even dashes, stroked like the graph's rails
/// (gaps at the top and bottom, where the rail leaves it), and a dashed rail in `lane` running
/// down into HEAD's row, where [`row`] continues it under HEAD's node.
pub fn worktree(t: &Theme, staged: bool, lane: Color) -> El {
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
    shape = dashed_rail(shape, PAD, RAIL_START, H, 0.0, lane);
    canvas(shape).w(52.0).h(H)
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

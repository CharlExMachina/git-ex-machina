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
    if !ups.is_empty() && !look.under_worktree {
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

/// The uncommitted row's mark: a dashed accent ring (`stroke-dasharray: 2.5 2`, 1.5 wide) and
/// a dashed lane-1 rail down to HEAD (`2 3`, 2 wide, round caps), in a 52 × 30 box.
pub fn worktree(t: &Theme, staged: bool) -> El {
    let c = [12.0, MID];
    let mut shape = Shape::new();
    if staged {
        shape = shape.fill(circle(c, 4.5), t.accent).stroke_with(circle(c, 4.5), true, 1.5, LineCap::Butt, t.accent);
    } else {
        for arc in dashed_circle(c, 4.5, 2.5, 2.0) {
            shape = shape.stroke_with(arc, false, 1.5, LineCap::Butt, t.accent);
        }
    }
    let mut y = 19.5;
    while y < H {
        shape = shape.line([12.0, y], [12.0, (y + 2.0).min(H)], STROKE, LineCap::Round, t.lane(1));
        y += 5.0;
    }
    canvas(shape).w(52.0).h(H)
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

/// The arcs an SVG circle stroked with `stroke-dasharray: dash gap` draws: starting at 3
/// o'clock and running clockwise.
fn dashed_circle(c: [f32; 2], r: f32, dash: f32, gap: f32) -> Vec<Vec<[f32; 2]>> {
    let length = std::f32::consts::TAU * r;
    let mut arcs = Vec::new();
    let mut s = 0.0;
    while s < length {
        let e = (s + dash).min(length);
        let steps = 6;
        arcs.push(
            (0..=steps)
                .map(|i| {
                    let a = (s + (e - s) * i as f32 / steps as f32) / r;
                    [c[0] + r * a.cos(), c[1] + r * a.sin()]
                })
                .collect(),
        );
        s += dash + gap;
    }
    arcs
}

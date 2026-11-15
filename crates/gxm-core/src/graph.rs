//! Lays out the commit graph: which lane each commit sits in and the lines between rows.
//!
//! Lanes keep their column for as long as they live, so a branch draws as a straight rail; a
//! freed column is reused by the next new lane. Each lane keeps the colour it was born with.

/// One row of the graph, for the commit at the same index.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GraphRow {
    /// The commit's column.
    pub lane: usize,
    /// The commit's colour (an index into the app's lane palette).
    pub color: usize,
    /// Lines drawn in this row.
    pub edges: Vec<Edge>,
    /// Columns this row needs, so the graph column can size itself.
    pub width: usize,
}

/// A line within one row. `Top` and `Bottom` are the row's edges; `Node` is the commit's dot in
/// the middle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edge {
    /// A lane passing by: from the top of `lane` straight to its bottom.
    Pass { lane: usize, color: usize },
    /// A child above coming down into this commit, from the top of `from`.
    In { from: usize, color: usize },
    /// This commit going down to a parent, at the bottom of `to`.
    Out { to: usize, color: usize },
}

#[derive(Clone, Debug)]
struct Lane {
    /// The commit this lane is waiting for.
    target: String,
    color: usize,
}

/// Lays out commits given newest first, children before parents, as `(id, parents)`.
pub fn layout<'a, I, P>(commits: I) -> Vec<GraphRow>
where
    I: IntoIterator<Item = (&'a str, P)>,
    P: IntoIterator<Item = &'a str>,
{
    let mut lanes: Vec<Option<Lane>> = Vec::new();
    let mut next_color = 0;
    let mut rows = Vec::new();
    for (id, parents) in commits {
        let mut edges = Vec::new();
        let hits: Vec<usize> = lanes
            .iter()
            .enumerate()
            .filter(|(_, l)| l.as_ref().is_some_and(|l| l.target == id))
            .map(|(i, _)| i)
            .collect();
        let (lane, color) = match hits.first() {
            Some(&i) => (i, lanes[i].as_ref().map_or(0, |l| l.color)),
            None => {
                let color = next_color;
                next_color += 1;
                (claim(&mut lanes), color)
            }
        };
        for (i, l) in lanes.iter().enumerate() {
            let Some(l) = l else { continue };
            if hits.contains(&i) {
                edges.push(Edge::In { from: i, color: l.color });
            }
        }
        for &h in &hits {
            lanes[h] = None;
        }
        let passing: Vec<usize> = (0..lanes.len()).filter(|&i| lanes[i].is_some()).collect();
        for (n, parent) in parents.into_iter().enumerate() {
            // A parent another lane already waits for: join that lane instead of opening a
            // parallel one.
            if let Some(k) = lanes.iter().position(|l| l.as_ref().is_some_and(|l| l.target == parent)) {
                let c = lanes[k].as_ref().map_or(color, |l| l.color);
                edges.push(Edge::Out { to: k, color: if n == 0 { color } else { c } });
                continue;
            }
            let (to, c) = if n == 0 {
                (lane, color)
            } else {
                let c = next_color;
                next_color += 1;
                (claim_after(&mut lanes, lane), c)
            };
            lanes[to] = Some(Lane { target: parent.to_owned(), color: c });
            edges.push(Edge::Out { to, color: c });
        }
        for i in passing {
            if let Some(l) = &lanes[i] {
                edges.push(Edge::Pass { lane: i, color: l.color });
            }
        }
        while lanes.last().is_some_and(Option::is_none) {
            lanes.pop();
        }
        let width = edges
            .iter()
            .map(|e| match *e {
                Edge::Pass { lane, .. } => lane,
                Edge::In { from, .. } => from,
                Edge::Out { to, .. } => to,
            })
            .chain([lane])
            .max()
            .map_or(1, |m| m + 1);
        rows.push(GraphRow { lane, color, edges, width });
    }
    rows
}

/// Which parts of a row belong to a set of commits, such as a pull request's.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lit {
    /// The row's commit is one of them.
    pub node: bool,
    /// For each of the row's edges, in order.
    pub edges: Vec<bool>,
}

/// Lights a set of commits (`member[i]` for row `i`) in a laid-out graph: their nodes, the
/// lines from each down to its parents, and the lane those lines run in until they land. A line
/// that joins a lane already running (the base branch) lights only itself, not that lane.
pub fn light(rows: &[GraphRow], member: &[bool]) -> Vec<Lit> {
    let mut lanes: Vec<usize> = Vec::new();
    let mut out = Vec::with_capacity(rows.len());
    for (i, row) in rows.iter().enumerate() {
        let mine = member.get(i).copied().unwrap_or(false);
        let edges = row
            .edges
            .iter()
            .map(|e| match *e {
                Edge::Pass { lane, .. } => lanes.contains(&lane),
                Edge::In { from, .. } => lanes.contains(&from),
                Edge::Out { .. } => mine,
            })
            .collect();
        let passing = |lane: usize| row.edges.iter().any(|e| matches!(*e, Edge::Pass { lane: l, .. } if l == lane));
        lanes.retain(|l| !row.edges.iter().any(|e| matches!(*e, Edge::In { from, .. } if from == *l)));
        if mine {
            for e in &row.edges {
                if let Edge::Out { to, .. } = *e
                    && !passing(to)
                    && !lanes.contains(&to)
                {
                    lanes.push(to);
                }
            }
        }
        out.push(Lit { node: mine, edges });
    }
    out
}

/// The first free column, or a new one.
fn claim(lanes: &mut Vec<Option<Lane>>) -> usize {
    match lanes.iter().position(Option::is_none) {
        Some(i) => i,
        None => {
            lanes.push(None);
            lanes.len() - 1
        }
    }
}

/// The first free column right of `after`, or a new one, so a merged branch opens to the right
/// of the commit that merged it.
fn claim_after(lanes: &mut Vec<Option<Lane>>, after: usize) -> usize {
    match lanes.iter().enumerate().skip(after + 1).find(|(_, l)| l.is_none()) {
        Some((i, _)) => i,
        None => {
            lanes.push(None);
            lanes.len() - 1
        }
    }
}

#[cfg(test)]
mod tests;

use super::*;

fn rows(commits: &[(&'static str, &[&'static str])]) -> Vec<GraphRow> {
    layout(commits.iter().map(|(id, ps)| (*id, ps.iter().copied())))
}

/// The demo workspace's history: a feature branch merged into main, and a fix branch that
/// forked from a tagged commit and is still open.
#[test]
fn branches_keep_their_rail_and_colour_and_rejoin_where_they_forked() {
    let g = rows(&[
        ("merge", &["sidebar", "virtualize"]),
        ("virtualize", &["lanes"]),
        ("evict", &["measure"]),
        ("sidebar", &["tag"]),
        ("lanes", &["start"]),
        ("measure", &["tag"]),
        ("tag", &["bump"]),
        ("start", &["bump"]),
        ("bump", &["root"]),
        ("root", &[]),
    ]);
    let placed: Vec<(usize, usize)> = g.iter().map(|r| (r.lane, r.color)).collect();
    assert_eq!(
        placed,
        [(0, 0), (1, 1), (2, 2), (0, 0), (1, 1), (2, 2), (0, 0), (1, 1), (0, 0), (0, 0)],
        "main stays in column 0, the merged branch opens to its right, the fix branch takes the next column"
    );

    assert_eq!(
        g[0].edges,
        [Edge::Out { to: 0, color: 0 }, Edge::Out { to: 1, color: 1 }],
        "a merge opens a lane for its second parent"
    );
    assert!(
        g[2].edges.contains(&Edge::Pass { lane: 0, color: 0 })
            && g[2].edges.contains(&Edge::Pass { lane: 1, color: 1 })
    );
    assert_eq!(g[2].width, 3);
    assert!(
        g[5].edges.contains(&Edge::Out { to: 0, color: 2 }),
        "the fix branch curves back into main in its own colour"
    );
    assert_eq!(g[6].width, 2, "and its column is freed after it rejoins");
    assert!(g[7].edges.contains(&Edge::Out { to: 0, color: 1 }), "the feature branch rejoins main where it forked");
    assert_eq!(g[9].edges, [Edge::In { from: 0, color: 0 }], "a root commit ends its lane");
}

#[test]
fn a_freed_column_is_reused_by_the_next_branch() {
    let g = rows(&[("a", &["c"]), ("b", &["c"]), ("c", &["d"]), ("e", &["d"]), ("d", &[])]);
    assert_eq!(g.iter().map(|r| r.lane).collect::<Vec<_>>(), [0, 1, 0, 1, 0]);
    assert_eq!(g[3].color, 2, "a new lane in a reused column takes a new colour");
}

//! The app's icons: a 16-point grid, stroked 1.5 in the current colour with round caps and
//! joins. Draw a new icon on the same grid and stroke.

use rune::Icon;

macro_rules! icon {
    ($id:ident, $name:literal, $parts:literal) => {
        pub static $id: Icon = Icon {
            name: $name,
            view: 16.0,
            body: concat!(
                r#"<g fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">"#,
                $parts,
                "</g>"
            ),
        };
    };
}

icon!(
    BRANCH,
    "gxm-branch",
    r#"<circle cx="5" cy="3.5" r="1.5"/><circle cx="5" cy="12.5" r="1.5"/><circle cx="11" cy="4.5" r="1.5"/><path d="M5 5v6"/><path d="M11 6c0 3.5-6 2.5-6 5"/>"#
);
icon!(SEARCH, "gxm-search", r#"<circle cx="7" cy="7" r="4.5"/><path d="M10.5 10.5 13.5 13.5"/>"#);
icon!(FETCH, "gxm-fetch", r#"<path d="M8 2.5v8"/><path d="M4.5 7 8 10.5 11.5 7"/><path d="M3 13.5h10"/>"#);
icon!(PULL, "gxm-pull", r#"<path d="M8 2.5v8"/><path d="M4.5 7 8 10.5 11.5 7"/><path d="M3 13.5h10"/>"#);
icon!(PUSH, "gxm-push", r#"<path d="M8 13.5v-8"/><path d="M4.5 9 8 5.5 11.5 9"/><path d="M3 2.5h10"/>"#);
icon!(TAG, "gxm-tag", r#"<path d="M2.5 2.5h5l6 6-5 5-6-6z"/><circle cx="5.5" cy="5.5" r="0.75"/>"#);
icon!(PLUS, "gxm-plus", r#"<path d="M8 3v10M3 8h10"/>"#);
icon!(PERSON, "gxm-person", r#"<circle cx="8" cy="5.5" r="2.5"/><path d="M3 13.5c0-2.6 2.2-4 5-4s5 1.4 5 4"/>"#);
icon!(CHECK, "gxm-check", r#"<path d="M3.5 8.5 6.5 11.5 12.5 4.5"/>"#);
icon!(SIDEBAR, "gxm-sidebar", r#"<rect x="2" y="3" width="12" height="10" rx="2"/><path d="M6 3v10"/>"#);
icon!(
    COPY,
    "gxm-copy",
    r#"<rect x="5.5" y="5.5" width="8" height="8" rx="1.5"/><path d="M10.5 5.5v-2a1 1 0 0 0-1-1h-6a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2"/>"#
);
icon!(COMMIT, "gxm-commit", r#"<circle cx="8" cy="8" r="2.5"/><path d="M1.5 8h4M10.5 8h4"/>"#);
icon!(
    MERGE,
    "gxm-merge",
    r#"<circle cx="4.5" cy="3.5" r="1.5"/><circle cx="4.5" cy="12.5" r="1.5"/><circle cx="11.5" cy="10" r="1.5"/><path d="M4.5 5v6"/><path d="M4.5 5c0 3 3 5 5.5 5"/>"#
);
icon!(CHANGES, "gxm-changes", r#"<path d="M8 2.5v6M5 5.5h6M5 12.5h6"/>"#);
icon!(
    PULL_REQUEST,
    "gxm-pull-request",
    r#"<circle cx="4.5" cy="3.5" r="1.5"/><circle cx="4.5" cy="12.5" r="1.5"/><circle cx="11.5" cy="12.5" r="1.5"/><path d="M4.5 5v6"/><path d="M11.5 11V6.5a2 2 0 0 0-2-2H7"/><path d="M8.5 3 7 4.5 8.5 6"/>"#
);
icon!(X, "gxm-x", r#"<path d="M4.5 4.5l7 7M11.5 4.5l-7 7"/>"#);
icon!(
    EYE,
    "gxm-eye",
    r#"<path d="M1.5 8s2.5-4.5 6.5-4.5S14.5 8 14.5 8s-2.5 4.5-6.5 4.5S1.5 8 1.5 8z"/><circle cx="8" cy="8" r="2"/>"#
);
icon!(CLOCK, "gxm-clock", r#"<circle cx="8" cy="8" r="5.5"/><path d="M8 5v3l2 1.5"/>"#);
icon!(
    COMMENT,
    "gxm-comment",
    r#"<path d="M2.5 4a1.5 1.5 0 0 1 1.5-1.5h8A1.5 1.5 0 0 1 13.5 4v5.5A1.5 1.5 0 0 1 12 11H7l-3 2.5V11a1.5 1.5 0 0 1-1.5-1.5z"/>"#
);
icon!(BACK, "gxm-back", r#"<path d="M9.5 3.5 5 8l4.5 4.5"/>"#);
icon!(
    EXTERNAL,
    "gxm-external",
    r#"<path d="M9 2.5h4.5V7"/><path d="M13.5 2.5 7.5 8.5"/><path d="M11.5 9.5v3a1 1 0 0 1-1 1h-7a1 1 0 0 1-1-1v-7a1 1 0 0 1 1-1h3"/>"#
);
icon!(CHEVRON, "gxm-chevron", r#"<path d="M4.5 6.5 8 10l3.5-3.5"/>"#);
icon!(CHEVRON_RIGHT, "gxm-chevron-right", r#"<path d="M6.5 4.5 10 8l-3.5 3.5"/>"#);
// An arrow down into a tray: take an update.
icon!(
    DOWNLOAD,
    "gxm-download",
    r#"<path d="M8 2.5v7.5"/><path d="M5 7 8 10l3-3"/><path d="M2.5 10.5v1.5a1.5 1.5 0 0 0 1.5 1.5h8a1.5 1.5 0 0 0 1.5-1.5v-1.5"/>"#
);
// A circling arrow: restart into an update.
icon!(RESTART, "gxm-restart", r#"<path d="M13.5 8a5.5 5.5 0 1 1-1.6-3.9"/><path d="M12.5 1.5v3h-3"/>"#);
// Two arrows chasing each other round: check for an update.
icon!(
    REFRESH,
    "gxm-refresh",
    r#"<path d="M13 6.5A5 5 0 0 0 3.6 5"/><path d="M3.5 2.5V5H6"/><path d="M3 9.5a5 5 0 0 0 9.4 1.5"/><path d="M12.5 13.5V11H10"/>"#
);

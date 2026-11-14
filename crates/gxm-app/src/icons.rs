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

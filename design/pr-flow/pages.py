from gen import *

PR_TITLE = 'Evict avatars by last paint, not insert'

def pr_header(compact=False):
    return (f'<div class="row" style="gap:10px"><span class="state st-open">{icon("pr", 12)}Open</span>'
            f'<span class="monosm faint">#131</span></div>'
            f'<div class="{"title" if compact else "title-lg"}" style="margin-top:8px">{PR_TITLE}</div>'
            f'<div class="row muted" style="margin-top:8px;font-size:12px;gap:6px;flex-wrap:wrap">{avatar("L", 4)}<span style="color:var(--ink);font-weight:500">Lea Brandt</span>'
            f'wants to merge 2 commits into {ref("head", "main")} from {ref("branch", "fix/avatar-cache", 4)}<span>· opened 5 hours ago</span></div>')

DESCRIPTION = ('<div class="md"><p>Avatars were evicted in the order they were loaded, so the ones on screen '
               'were often the first to go and flickered back in while scrolling.</p>'
               '<ul><li>Evict by <code>last_paint</code> instead of insertion order</li>'
               '<li>Count hits, misses and evictions in <code>AvatarMetrics</code></li></ul>'
               '<p class="muted" style="margin:0">Fixes <span style="color:var(--accent)">#117</span> · History stutters past 10k commits</p></div>')

def merge_box(wide=True):
    return (f'<div class="card" style="padding:14px 14px 12px">'
            f'<div class="row" style="gap:10px;align-items:flex-start"><span class="bad" style="margin-top:1px">{icon("x", 16)}</span>'
            f'<div class="col grow"><span style="font-weight:600">1 check failing</span><span class="muted" style="font-size:12px">journeys · first_look failed at step 20</span></div></div>'
            f'<div class="row" style="gap:10px;align-items:flex-start;margin-top:10px"><span class="ok" style="margin-top:1px">{icon("check", 16)}</span>'
            f'<div class="col grow"><span style="font-weight:600">No conflicts with main</span><span class="muted" style="font-size:12px">1 approval · your review is requested</span></div></div>'
            f'<div class="row" style="margin-top:12px">{btn("Squash and merge", "merge", "primary", extra="disabled")}{btn("", "chevron", "secondary", extra="disabled")}</div></div>')

# ---------------------------------------------------------------- inspector proposal
def inspector_commit(kind):
    if kind == 'open':
        title, who, date, ago, sha, parent = PR_TITLE, LEA, 'Nov 11, 2026, 10:00', '5 hours ago', '1cabd07c', '8f566f4'
        pr = (f'<span class="state st-open">{icon("pr", 12)}Open</span>'
              f'<span class="grow" style="min-width:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis"><span class="monosm faint">#131</span> Evict avatars by last paint</span>')
        body = 'Avatars that are on screen stay cached even when they were loaded first.'
        kicker = 'Commit'
    else:
        title, who, date, ago, sha, parent = 'Merge feature/graph-virtualization', ANA, 'Nov 11, 2026, 13:00', '2 hours ago', '5832000b', 'bd3b72a · c0e8db2'
        pr = (f'<span class="state st-merged">{icon("merge", 12)}Merged</span>'
              f'<span class="grow" style="min-width:0;white-space:nowrap;overflow:hidden;text-overflow:ellipsis"><span class="monosm faint">#128</span> Virtualize commit rows past 10k</span>')
        body = 'Brings row virtualization into main. No conflicts.'
        kicker = 'Head of main'
    files = (file_row('M', 'src/avatars/cache.rs', 27, 31, True) + file_row('A', 'src/avatars/metrics.rs', 54, 0)) if kind == 'open' else (
        file_row('M', 'src/graph/rows.rs', 84, 21, True) + file_row('A', 'src/graph/virtual_list.rs', 142, 0) + file_row('M', 'src/sidebar.rs', 6, 9))
    return (f'<aside class="insp" style="height:640px"><div class="bar" style="padding:0 12px 0 16px">'
            f'<div class="tabs"><span class="tab">Message</span><span class="tab on">Files</span></div><div class="grow"></div>{btn("", "copy", "ghost")}</div>'
            f'<div class="col" style="padding:20px;gap:18px">'
            f'<div class="col" style="gap:8px"><span class="label">{kicker}</span><span class="title">{title}</span></div>'
            f'<div class="kv">'
            f'<span class="k">Author</span><span class="row" style="gap:8px">{avatar(who[0], who[1])}<span style="font-weight:500">{who[2]}</span></span>'
            f'<span class="k">Date</span><span>{date} <span class="muted">· {ago}</span></span>'
            f'<span class="k">Commit</span><span class="row" style="gap:6px"><span class="chip">{sha}</span></span>'
            f'<span class="k">{"Parents" if "·" in parent else "Parent"}</span><span class="monosm muted">{parent}</span>'
            f'<span class="k">Pull request</span><span class="row" style="gap:8px;min-width:0">{pr}</span>'
            f'</div>'
            f'<div>{btn("See pull request", "pr", "secondary")}</div>'
            f'<p class="muted" style="margin:0">{body}</p>'
            f'<div class="col" style="gap:2px"><div class="row" style="justify-content:space-between;padding:0 8px 6px"><span class="label">Changed files</span>'
            f'<span class="monosm muted">{"2 files · +81 −31" if kind == "open" else "4 files · +232 −93"}</span></div>{files}</div></div></aside>')

def page_inspector():
    left = f'<div style="position:relative">{inspector_commit("open")}</div>'
    right = f'<div style="position:relative">{inspector_commit("merged")}</div>'
    return (BASE.replace('width:1552px;', 'width:784px;') +
            f'<div class="row" style="gap:16px;align-items:flex-start"><div class="col" style="gap:8px"><span class="label">A commit in an open PR</span>'
            f'<div style="border-radius:14px;overflow:hidden;box-shadow:0 0 0 1px var(--line)">{left}</div></div>'
            f'<div class="col" style="gap:8px"><span class="label">A merge commit from a PR</span>'
            f'<div style="border-radius:14px;overflow:hidden;box-shadow:0 0 0 1px var(--line)">{right}</div></div></div>' + END)

# ---------------------------------------------------------------- A: the PR page
def page_a():
    tabs = (f'<div class="tabs"><span class="tab on">{icon("comment", 13)}Conversation<span class="count">4</span></span>'
            f'<span class="tab">{icon("commit", 13)}Commits<span class="count">2</span></span>'
            f'<span class="tab">{icon("check", 13)}Checks<span class="count" style="color:var(--removed)">3/4</span></span>'
            f'<span class="tab">{icon("file", 13)}Files<span class="count">2</span></span></div>')
    timeline = (
        f'<div class="tl">'
        f'<div class="tli"><span class="node">{avatar("L", 4, 24)}</span><div class="card" style="padding:12px 14px">'
        f'<div class="row muted" style="font-size:12px;margin-bottom:8px"><span style="color:var(--ink);font-weight:500">Lea Brandt</span>opened this · 5 hours ago</div>{DESCRIPTION}</div></div>'
        f'<div class="tli"><span class="node">{icon("commit", 13)}</span><div class="col" style="gap:4px;padding-top:3px">'
        f'<span class="muted" style="font-size:12px">Lea Brandt added 2 commits</span>'
        f'<div class="row" style="gap:10px"><span class="monosm faint">8f566f4</span><span>Measure avatar cache pressure</span></div>'
        f'<div class="row" style="gap:10px"><span class="monosm faint">1cabd07</span><span>{PR_TITLE}</span><span class="bad">{icon("x", 12)}</span></div></div></div>'
        f'<div class="tli"><span class="node ok">{icon("check", 13)}</span><div class="card" style="overflow:hidden">'
        f'<div class="row" style="padding:10px 14px;font-size:12px"><span style="font-weight:500">Kenji Mori</span><span class="ok">approved</span><span class="muted">· 3 hours ago</span></div>'
        f'<div class="gu-hunk" style="border-radius:0;box-shadow:none;border-top:1px solid var(--line)"><div class="gu-hunk-head">src/avatars/cache.rs</div>{hunk_lines(EVICT_HUNK[:4])}</div>'
        f'<div class="thread"><div class="row" style="gap:8px;align-items:flex-start">{avatar("K", 2)}<div class="col"><span style="font-size:12px"><b style="font-weight:500">Kenji Mori</b> <span class="muted">· 3h</span></span>'
        f'<span>Does <code class="mono" style="font-size:11.5px">pop_min</code> stay O(log n) once the heap holds 10k avatars?</span></div></div>'
        f'<div class="row" style="gap:8px;align-items:flex-start;margin-top:10px">{avatar("L", 4)}<div class="col"><span style="font-size:12px"><b style="font-weight:500">Lea Brandt</b> <span class="muted">· 2h</span></span>'
        f'<span>Yes, it is a binary heap keyed by paint time. Added a bench in the metrics module.</span></div></div></div></div></div>'
        f'<div class="tli"><span class="node bad">{icon("x", 13)}</span><div style="padding-top:3px"><span class="bad" style="font-weight:500">journeys · first_look</span> '
        f'<span class="muted">failed at step 20: "Evict avatars…" isn\'t in [inspector-title] · 1 hour ago</span></div></div>'
        f'<div class="tli"><span class="node">{avatar("A", 3, 24)}</span><div class="composer" style="min-height:0">Leave a comment, or review the changes…</div>'
        f'<div class="row" style="margin-top:8px;justify-content:flex-end">{btn("Comment", None, "secondary")}{btn("Review changes", "eye", "primary")}</div></div>'
        f'</div>')
    main = (f'<main class="main"><div class="bar">{btn("History", "back", "ghost")}<span class="faint">/</span><span class="muted">Pull requests</span>'
            f'<div class="grow"></div>{btn("Check out branch", "branch")}{btn("Open on GitHub", "external", "ghost")}</div>'
            f'<div style="padding:16px 24px 12px;border-bottom:1px solid var(--line)">{pr_header()}</div>'
            f'<div class="row" style="padding:10px 24px;border-bottom:1px solid var(--line)">{tabs}</div>'
            f'<div style="padding:18px 24px 0 24px;overflow:hidden;flex:1">{timeline}</div></main>')
    insp = (f'<aside class="insp"><div class="bar" style="padding:0 12px 0 16px"><span class="label">Pull request</span><div class="grow"></div>{btn("", "copy", "ghost")}</div>'
            f'<div class="col" style="padding:20px;gap:20px">{merge_box()}'
            f'<div class="col" style="gap:2px"><div class="label" style="padding:0 8px 6px">Reviewers</div>{reviewers()}</div>'
            f'<div class="col" style="gap:2px"><div class="label" style="padding:0 8px 6px">Checks</div>{checks_list()}</div>'
            f'<div class="col" style="gap:8px"><div class="label" style="padding:0 8px">Labels</div><div class="row" style="padding:0 8px;gap:6px">'
            f'{ref("branch", "performance", 3)}{ref("branch", "avatars", 4)}</div></div>'
            f'<div class="col" style="gap:8px"><div class="label" style="padding:0 8px">Linked issue</div>'
            f'<div class="row" style="padding:0 8px;gap:8px"><span class="ok">{icon("issue", 14)}</span><span class="monosm faint">#117</span><span>History stutters past 10k commits</span></div></div>'
            f'</div></aside>')
    return BASE + f'<div class="win">{sidebar()}{main}{insp}</div>' + END

# ---------------------------------------------------------------- B: review cockpit
def page_b():
    files_side = (f'<aside class="side"><div class="bar" style="padding:0 12px"><div class="lights"><i style="background:#ff5f57"></i><i style="background:#febc2e"></i><i style="background:#28c840"></i></div>'
                  f'<div class="grow"></div>{btn("", "sidebar", "ghost")}</div>'
                  f'<div style="padding:14px 16px 10px">{btn("atlas-web", "back", "ghost")}</div>'
                  f'<div style="padding:0 16px 14px"><div class="row" style="gap:8px"><span class="state st-open">{icon("pr", 12)}Open</span><span class="monosm faint">#131</span></div>'
                  f'<div style="font-weight:600;margin-top:8px">{PR_TITLE}</div><div class="muted" style="font-size:12px;margin-top:4px">fix/avatar-cache → main</div></div>'
                  f'<div class="row" style="padding:0 16px 8px;justify-content:space-between"><span class="label">Files</span><span class="monosm muted">1 of 2 viewed</span></div>'
                  f'<div style="padding:0 16px 12px"><div style="height:4px;border-radius:2px;background:var(--well)"><div style="width:50%;height:4px;border-radius:2px;background:var(--added)"></div></div></div>'
                  f'<div class="col" style="padding:0 8px">'
                  f'<div class="gu-file" aria-selected="true"><span class="gu-status gu-status-M">M</span><span class="gu-file-path"><span class="gu-file-name">cache.rs</span></span><span class="row muted" style="gap:4px;font-size:11px">{icon("comment", 12)}2</span></div>'
                  f'<div class="gu-file"><span class="gu-status gu-status-A">A</span><span class="gu-file-path"><span class="gu-file-name">metrics.rs</span></span><span class="ok">{icon("check", 13)}</span></div></div>'
                  f'<div class="row" style="padding:16px 16px 8px;justify-content:space-between"><span class="label">Commits</span><span class="monosm muted">all</span></div>'
                  f'<div class="col" style="padding:0 16px;gap:6px;font-size:12px"><div class="row"><span class="monosm faint">8f566f4</span><span class="muted">Measure avatar cache…</span></div>'
                  f'<div class="row"><span class="monosm faint">1cabd07</span><span class="muted">Evict avatars by last…</span></div></div>'
                  f'<div style="margin-top:auto;padding:12px;border-top:1px solid var(--line)" class="row faint"><span class="gu-kbd">j</span><span class="gu-kbd">k</span><span style="font-size:11.5px">next / previous change</span></div></aside>')
    diff_lines = [
        ('ctx', 58, 58, 'impl AvatarCache {'),
        ('ctx', 59, 59, '    fn evict(&amp;mut self) {'),
        ('ctx', 60, 60, '        let before = self.bytes;'),
    ] + [(t, a, b, '    ' + x) for t, a, b, x in EVICT_HUNK] + [
        ('add', '', 66, '        self.metrics.flush();'),
        ('ctx', 65, 67, '    }'),
        ('ctx', 66, 68, ''),
        ('ctx', 67, 69, '    fn insert(&amp;mut self, id: AvatarId, img: Image) {'),
        ('del', 68, '', '        self.by_insert.push_back(id);'),
        ('add', '', 70, '        self.by_last_paint.push(id, self.clock.now());'),
        ('ctx', 69, 71, '        self.bytes += img.bytes();'),
    ]
    first = hunk_lines(diff_lines[:8])
    rest = hunk_lines(diff_lines[8:])
    thread = (f'<div class="thread" style="margin:0;border-bottom:1px solid var(--line)"><div class="row" style="gap:8px;align-items:flex-start">{avatar("K", 2)}<div class="col grow"><span style="font-size:12px"><b style="font-weight:500">Kenji Mori</b> <span class="muted">· line 63 · 3h</span></span>'
              f'<span>Does <code class="mono" style="font-size:11.5px">pop_min</code> stay O(log n) once the heap holds 10k avatars?</span></div></div>'
              f'<div class="row" style="gap:8px;align-items:flex-start;margin-top:10px">{avatar("L", 4)}<div class="col grow"><span style="font-size:12px"><b style="font-weight:500">Lea Brandt</b> <span class="muted">· 2h</span></span>'
              f'<span>Yes, a binary heap keyed by paint time. There\'s a bench in metrics.rs.</span></div></div>'
              f'<div class="composer" style="margin-top:10px;min-height:0;padding:8px 12px">Reply…</div></div>')
    main = (f'<main class="main"><div class="bar"><div class="tabs"><span class="tab on">Unified</span><span class="tab">Split</span></div>'
            f'<div class="grow"></div><span class="faint" style="font-size:11.5px">2 comments · 1 unresolved</span>{btn("Check out", "branch")}{btn("Open on GitHub", "external", "ghost")}</div>'
            f'<div class="row" style="padding:10px 16px;border-bottom:1px solid var(--line);background:var(--chrome)"><span class="gu-status gu-status-M">M</span>'
            f'<span class="mono"><span class="muted">src/avatars/</span>cache.rs</span><span class="gu-file-stat" style="margin-left:6px"><span class="add">+27</span><span class="del">−31</span></span>'
            f'<div class="grow"></div><label class="row muted" style="gap:6px;font-size:12px"><span style="width:14px;height:14px;border-radius:4px;box-shadow:inset 0 0 0 1.5px var(--edge)"></span>Viewed</label></div>'
            f'<div style="padding:14px 16px;overflow:hidden;flex:1"><div class="gu-hunk"><div class="gu-hunk-head">@@ -58,12 +58,14 @@ impl AvatarCache</div>{first}{thread}{rest}</div>'
            f'<div class="gu-hunk" style="margin-top:12px"><div class="gu-hunk-head">@@ -102,3 +104,5 @@ fn report(&amp;self)</div>'
            + hunk_lines([('ctx', 102, 104, 'fn report(&amp;self) -&gt; Report {'), ('add', '', 105, '    let m = self.metrics.snapshot();'), ('add', '', 106, '    Report { hits: m.hits, misses: m.misses, ..Report::default() }'), ('ctx', 103, 107, '}')]) +
            f'</div></div></main>')
    insp = (f'<aside class="insp"><div class="bar" style="padding:0 12px 0 16px"><span class="label">Review</span><div class="grow"></div>{btn("", "copy", "ghost")}</div>'
            f'<div class="col" style="padding:20px;gap:18px">'
            f'<div class="col" style="gap:6px"><div class="row" style="gap:8px">{avatar("L", 4)}<span style="font-weight:500">Lea Brandt</span><span class="muted" style="font-size:12px">opened 5 hours ago</span></div>'
            f'<div class="muted" style="font-size:12.5px;line-height:18px">Avatars were evicted in load order, so on-screen ones flickered back in while scrolling. Evicts by last paint and counts hits, misses and evictions.</div></div>'
            f'<div class="card row" style="padding:10px 12px;gap:10px"><span class="bad">{icon("x", 14)}</span><span class="grow" style="font-size:12px">journeys · first_look failed</span><span class="monosm faint">3/4</span></div>'
            f'<div class="col" style="gap:2px"><div class="label" style="padding:0 8px 6px">Reviewers</div>{reviewers()}</div>'
            f'<div class="col" style="gap:10px"><div class="label">Your review</div>'
            f'<div class="composer" style="min-height:88px">Summarise your review… <span class="faint">(2 pending comments will be sent with it)</span></div>'
            f'<div class="tabs" style="align-self:flex-start"><span class="tab">Comment</span><span class="tab on" style="color:var(--added)">{icon("check", 13)}Approve</span><span class="tab">Request changes</span></div>'
            f'<div class="row">{btn("Submit review", None, "primary", badge="⌘↩")}</div></div>'
            f'</div></aside>')
    return BASE + f'<div class="win">{files_side}{main}{insp}</div>' + END

# ---------------------------------------------------------------- C: PR on the graph
def page_c():
    rows = [
        ('', 'Uncommitted changes', '', '', '', True),
        (ref('head', 'main') + ref('remote', 'origin/main', 1), 'Merge feature/graph-virtualization', 'A', '2h', '5832000', True),
        (ref('branch', 'feature/graph-virtualization', 2), 'Virtualize commit rows past 10k', 'K', '3h', 'c0e8db2', True),
        ('PR', '', '', '', '', False),
        (ref('branch', 'fix/avatar-cache', 4), PR_TITLE, 'L', '5h', '1cabd07', False),
        ('', 'Spring the sidebar collapse', 'A', 'Yesterday', 'bd3b72a', True),
        ('', 'Draw lanes with cubic joins', 'K', 'Mon', '4f7549e', True),
        ('', 'Measure avatar cache pressure', 'L', 'Mon', '8f566f4', False),
        (ref('tag', 'v0.4.0'), 'Fix avatar cache eviction', 'L', 'Mon', '76245e5', True),
        ('', 'Start graph virtualization', 'K', 'Nov 8', '9c902d7', True),
        ('', 'Bump rune to 0.9', 'A', 'Nov 7', '5b906db', True),
        ('', 'Tune brass for the light theme', 'K', 'Nov 6', '4166440', True),
        ('', 'Split view remembers column widths', 'A', 'Nov 5', '6572a47', True),
        ('', 'Read refs with gix instead of shelling out', 'L', 'Nov 4', '8c1088a', True),
        ('', 'Window chrome and traffic-light inset', 'A', 'Nov 3', '8e104fc', True),
        ('', 'Persist pinned repositories', 'K', 'Nov 2', 'c3d1863', True),
        ('', 'Watch .git for external changes', 'L', 'Nov 2', '50311b1', True),
        ('', 'Group repositories by owner', 'A', 'Nov 1', '40e248d', True),
        ('', 'Diff view wraps at 100 columns', 'K', 'Oct 31', '91232c0', True),
        ('', 'Load history in pages of 500 behind the scenes', 'L', 'Oct 30', 'b13bbf7', True),
        ('', 'Ahead and behind counts per repository', 'A', 'Oct 29', '24647fd', True),
        ('', 'Cache lane layout between fetches', 'K', 'Oct 29', '36627ee', True),
        ('', 'Recessed search field in the sidebar', 'L', 'Oct 28', 'c866fc8', True),
    ]
    names = {'A': ('A', 3, 'Ana Ruiz'), 'K': ('K', 2, 'Kenji Mori'), 'L': ('L', 4, 'Lea Brandt')}
    lanes_x = [20, 34, 48]
    out = ''
    for refs, subj, who, date, sha, dim in rows:
        if refs == 'PR':
            out += (f'<div class="row" style="height:30px;padding:0 12px 0 60px;margin:2px 0;border-radius:6px;background:color-mix(in srgb,var(--lane-4) 12%,transparent);box-shadow:inset 0 0 0 1px color-mix(in srgb,var(--lane-4) 40%,transparent)">'
                    f'<span style="color:var(--lane-4)">{icon("pr", 14)}</span><span style="font-weight:600">#131</span><span class="muted">2 commits · Lea Brandt · review requested from you</span>'
                    f'<span class="grow"></span><span class="bad row" style="gap:4px;font-size:12px">{icon("x", 12)}1 failing</span><span class="ok row" style="gap:4px;font-size:12px;margin-left:10px">{icon("check", 12)}1 approval</span></div>')
            continue
        avatar_html = avatar(*names[who][:2]) if who else ''
        out += (f'<div class="row {"dim" if dim else ""}" style="height:30px;padding-right:12px;gap:0">'
                f'<span style="width:60px;flex:none"></span><span class="row grow" style="gap:6px">{refs}<span style="font-weight:{400 if subj.startswith("Merge") else 500};{"color:var(--ink-muted)" if subj.startswith("Merge") else ""}">{subj}</span></span>'
                f'<span class="row" style="width:128px;gap:6px;font-size:12px;color:var(--ink-muted)">{avatar_html}{names[who][2] if who else ""}</span>'
                f'<span style="width:64px;text-align:right;font-size:12px;color:var(--ink-muted)">{date}</span>'
                f'<span style="width:60px;text-align:right" class="monosm faint">{sha}</span></div>')
    def cy(i):
        return 15 + 30 * i + (34 if i >= 3 else 0)
    X = {1: 20, 2: 34, 4: 48}
    lane_of = {0: 1, 1: 1, 2: 2, 3: 4, 4: 1, 5: 2, 6: 4, 7: 1, 8: 2}
    paths, nodes = [], []
    n_rows = 21
    dim_g, pr_g = [], []
    # main lane, feature lane, PR lane
    dim_g.append(f'<path d="M20 {cy(1)}V{cy(n_rows) + 30}" stroke="var(--lane-1)"/>')
    dim_g.append(f'<path d="M20 {cy(1)}C20 {cy(1)+9} 34 {cy(1)+5} 34 {cy(1)+15}V{cy(8)}" stroke="var(--lane-2)"/>')
    dim_g.append(f'<path d="M34 {cy(8)}C34 {cy(8)+9} 20 {cy(8)+5} 20 {cy(9)}" stroke="var(--lane-2)"/>')
    pr_g.append(f'<path d="M48 {cy(3)}V{cy(6)}C48 {cy(6)+9} 20 {cy(6)+5} 20 {cy(7)}" stroke="var(--lane-4)"/>')
    for i in range(1, n_rows):
        lane = lane_of.get(i, 1)
        x = X[lane]
        r = 3.25 if i == 1 else 4.5
        dot = f'<circle cx="{x}" cy="{cy(i)}" r="6.5" fill="var(--canvas)"/><circle cx="{x}" cy="{cy(i)}" r="{r}" fill="var(--lane-{lane})"/>'
        (pr_g if lane == 4 else dim_g).append(dot)
    dim_g.append(f'<circle cx="20" cy="{cy(0)}" r="4.5" fill="none" stroke="var(--accent)" stroke-width="1.5" stroke-dasharray="2.5 2"/>')
    graph = (f'<svg width="60" height="720" style="position:absolute;left:8px;top:6px" fill="none" stroke-width="2" stroke-linecap="round">'
             f'<g opacity=".38">{"".join(dim_g)}</g>{"".join(pr_g)}</svg>')
    main = (f'<main class="main"><div class="bar">{btn("Review", "eye", "primary")}{btn("Check out", "branch")}{btn("Squash and merge", "merge", extra="disabled")}'
            f'<div class="grow"></div>{btn("Show all history", None, "ghost")}</div>'
            f'<div class="row" style="padding:10px 16px 10px 24px;border-bottom:1px solid var(--line);font-size:12px;gap:6px"><span style="color:var(--lane-4)">{icon("pr", 12)}</span>'
            f'<span style="font-weight:500">Pull request #131</span><span class="muted">· fix/avatar-cache → main · highlighted in the history</span></div>'
            f'<div class="row" style="height:24px;padding:0 12px 0 58px;margin:0 8px;border-bottom:1px solid var(--line);font-size:11px;color:var(--ink-faint)"><span class="grow">Description</span>'
            f'<span style="width:128px">Author</span><span style="width:64px;text-align:right">Date</span><span style="width:60px;text-align:right">Commit</span></div>'
            f'<div style="position:relative;padding:6px 8px 0;overflow:hidden;flex:1">{graph}{out}</div></main>')
    insp = (f'<aside class="insp"><div class="bar" style="padding:0 12px 0 16px"><div class="tabs"><span class="tab on">Overview</span><span class="tab">Checks</span><span class="tab">Conversation<span class="count">4</span></span></div>'
            f'<div class="grow"></div>{btn("", "external", "ghost")}</div>'
            f'<div class="col" style="padding:20px;gap:18px"><div class="col" style="gap:8px"><span class="label">Pull request</span>{pr_header(compact=True)}</div>'
            f'<div class="muted" style="font-size:12.5px">{DESCRIPTION}</div>'
            f'{merge_box()}'
            f'<div class="col" style="gap:2px"><div class="label" style="padding:0 8px 6px">Reviewers</div>{reviewers()}</div>'
            f'</div></aside>')
    return BASE + f'<div class="win">{sidebar()}{main}{insp}</div>' + END

if __name__ == '__main__':
    for name, fn in [('inspector', page_inspector), ('a', page_a), ('b', page_b), ('c', page_c)]:
        open(f'{OUT}/{name}.html', 'w').write(fn())
    print('ok')

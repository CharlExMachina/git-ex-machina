"""States the C + B flow needs that its frames don't draw, in the same parts: drafts in the
review cockpit, Split, Viewed ticked (and the shelf it goes to), the inspector's Checks and Conversation tabs, and a
repository whose provider isn't connected. Proposed 2026-11-14 with the first build; they need
the person's sign-off. Run `python3 extras.py` here."""
from pages import *

def thread_comment(p, meta, body, pending=False, top=False):
    pill = ('<span style="font-family:var(--font-mono);font-size:11px;line-height:16px;padding:0 5px;border-radius:999px;'
            'background:var(--accent-soft);color:var(--accent)">Pending</span>') if pending else ''
    return (f'<div class="row" style="gap:8px;align-items:flex-start;{"" if top else "margin-top:10px"}">{avatar(p[0], p[1])}<div class="col grow">'
            f'<span class="row" style="gap:4px;font-size:12px"><b style="font-weight:500">{p[2]}</b><span class="muted">{meta}</span>{pill}</span>'
            f'<span>{body}</span></div></div>')

def drafts_panel():
    """A pending reply in a thread, a pending comment on a line, and the composer open on another."""
    thread = (f'<div class="thread" style="margin:0;border-bottom:1px solid var(--line)">'
              + thread_comment(KENJI, '· line 5 · 3h', 'Does <code class="mono" style="font-size:11.5px">pop_min</code> stay O(log n) once the heap holds 10k avatars?', top=True)
              + thread_comment(LEA, '· 2h', "Yes, a binary heap keyed by paint time. There's a bench in metrics.rs.")
              + thread_comment(ANA, '·', 'Thanks, the bench in metrics.rs settles it.', pending=True)
              + '<div class="composer" style="margin-top:10px;min-height:0;padding:8px 12px">Reply…</div></div>')
    pending = (f'<div class="thread" style="margin:0;border-bottom:1px solid var(--line)">'
               + thread_comment(ANA, '· line 6 ·', 'Could this flush the metrics too?', pending=True, top=True) + '</div>')
    composer = (f'<div class="thread" style="margin:0;border-bottom:1px solid var(--line)"><div class="row" style="gap:8px;align-items:flex-start">{avatar("A", 3)}'
                f'<div class="composer grow" style="min-height:0;padding:8px 12px;color:var(--ink)">Keep the old path behind a flag?<span style="border-left:1.5px solid var(--focus);margin-left:1px"></span></div></div></div>')
    lines = [('add', '', 5, '    let oldest = self.by_last_paint.pop_min();')]
    return (f'<div class="gu-hunk"><div class="gu-hunk-head">@@ -2,37 +2,33 @@</div>{hunk_lines(lines)}{thread}'
            f'{hunk_lines([("add", "", 6, "    self.metrics.evicted(oldest.bytes);")])}{pending}'
            f'{hunk_lines([("add", "", 7, "    self.remove(oldest);")])}{composer}</div>'
            f'<p class="muted" style="font-size:12px;margin:10px 0 0">Pressing a line opens the composer under it; Return keeps the comment as a draft '
            f'(Pending) until Submit review sends them all as one review.</p>')

def split_panel():
    def cell(t, n, text, right):
        cls = {'add': ' gu-hunk-add', 'del': ' gu-hunk-del', 'ctx': '', 'none': ''}[t]
        sign = {'add': '+', 'del': '−', 'ctx': ' ', 'none': ''}[t]
        border = 'border-left:1px solid var(--line);' if right else ''
        return (f'<div class="gu-hunk-line{cls}" style="grid-template-columns:36px 16px 1fr;flex:1;min-width:0;{border}">'
                f'<span class="gu-hunk-no">{n}</span><span class="gu-hunk-sign">{sign}</span><span class="gu-hunk-text">{text}</span></div>')
    rows = [(('ctx', 61, 'while self.bytes &gt; self.budget {'), ('ctx', 61, 'while self.bytes &gt; self.budget {')),
            (('del', 62, '    let oldest = self.by_insert.pop_front();'), ('add', 62, '    let oldest = self.by_last_paint.pop_min();')),
            (('none', '', ''), ('add', 63, '    self.metrics.evicted(oldest.bytes);')),
            (('ctx', 63, '    self.remove(oldest);'), ('ctx', 64, '    self.remove(oldest);'))]
    body = ''.join(f'<div style="display:flex">{cell(*a, False)}{cell(*b, True)}</div>' for a, b in rows)
    return f'<div class="gu-hunk"><div class="gu-hunk-head">@@ -58,12 +58,14 @@ impl AvatarCache</div>{body}</div>'

def viewed_panel():
    box = lambda on: (f'<span style="width:14px;height:14px;border-radius:4px;display:grid;place-items:center;'
                      + ('background:var(--added);color:var(--on-lane)">' + icon('check', 10) if on else 'box-shadow:inset 0 0 0 1.5px var(--edge)">') + '</span>')
    head = lambda on: (f'<div class="row" style="padding:10px 16px;border-bottom:1px solid var(--line);background:var(--chrome)"><span class="gu-status gu-status-A">A</span>'
                       f'<span class="mono"><span class="muted">src/avatars/</span>metrics.rs</span><span class="gu-file-stat" style="margin-left:6px"><span class="add">+54</span></span>'
                       f'<div class="grow"></div><label class="row muted" style="gap:6px;font-size:12px">{box(on)}Viewed</label></div>')
    file = lambda st, name, side, sel=False, muted=False: (
        f'<div class="gu-file" aria-selected="{str(sel).lower()}"><span class="gu-status gu-status-{st}">{st}</span>'
        f'<span class="gu-file-path"><span class="gu-file-name"{" style=\"color:var(--ink-muted)\"" if muted else ""}>{name}</span></span>{side}</div>')
    # Ticked, the file leaves the list for the Viewed shelf beneath it (folded until opened, as
    # T3 Code keeps settled threads), and the next file still to view opens.
    shelf = (f'<div class="row muted" style="margin:4px 8px 0;height:32px;padding:0 8px;gap:8px;border-radius:6px">'
             f'<span style="font-size:11px;font-weight:600;letter-spacing:.06em">Viewed</span>'
             f'<span style="flex:1;height:1px;background:var(--line)"></span><span class="monosm">1</span>{icon("chevron", 12)}</div>')
    side = (f'<aside class="side" style="height:auto;width:248px;padding:14px 0;border:1px solid var(--line);border-radius:10px">'
            f'<div class="row" style="padding:0 16px 8px;justify-content:space-between"><span class="label">Files</span><span class="monosm muted">1 of 2 viewed</span></div>'
            f'<div style="padding:0 16px 12px"><div style="height:4px;border-radius:2px;background:var(--well)"><div style="width:50%;height:4px;border-radius:2px;background:var(--added)"></div></div></div>'
            f'<div class="col" style="padding:0 8px">{file("M", "cache.rs", f"<span class=\"row muted\" style=\"gap:4px;font-size:11px\">{icon("comment", 12)}2</span>", sel=True)}</div>'
            f'{shelf}<div class="col" style="padding:0 8px">{file("A", "metrics.rs", f"<span class=\"ok\">{icon("check", 13)}</span>", muted=True)}</div></aside>')
    return (f'<div class="row" style="gap:16px;align-items:flex-start"><div class="cardc grow" style="overflow:hidden">{head(False)}{head(True)}</div>{side}</div>')

def tabs_bar(on, count):
    tab = lambda n, c=None: f'<span class="tab{" on" if n == on else ""}">{n}{f"<span class=count>{c}</span>" if c else ""}</span>'
    return (f'<div class="bar" style="padding:0 12px 0 16px"><div class="tabs">{tab("Overview")}{tab("Checks")}{tab("Conversation", count)}</div>'
            f'<div class="grow"></div>{btn("", "external", "ghost")}</div>')

def checks_panel():
    return (f'<aside class="insp" style="height:340px">{tabs_bar("Checks", 4)}<div class="col" style="padding:20px">'
            f'<div class="col" style="gap:2px"><div class="row" style="justify-content:space-between;padding:0 8px 6px"><span class="label">Checks</span>'
            f'<span class="monosm muted">3 of 4 passed</span></div>{checks_list()}</div></div></aside>')

def conversation_panel():
    entry = lambda p, did, body='': (f'<div class="col" style="gap:2px"><div class="row" style="gap:6px;font-size:12px">{avatar(p[0], p[1])}'
                                     f'<span style="font-weight:500">{p[2]}</span><span class="muted">{did}</span></div>'
                                     + (f'<div style="padding-left:22px">{body}</div>' if body else '') + '</div>')
    thread = (f'<div class="card col" style="padding:12px;gap:10px"><span class="monosm muted" style="font-weight:400">src/avatars/cache.rs:5</span>'
              + entry(KENJI, '3h', 'Does <code class="mono" style="font-size:11.5px;background:var(--well);padding:1px 4px;border-radius:4px">pop_min</code> stay O(log n) once the heap holds 10k avatars?')
              + entry(LEA, '2h', "Yes, a binary heap keyed by paint time. There's a bench in metrics.rs.") + '</div>')
    return (f'<aside class="insp" style="height:520px">{tabs_bar("Conversation", 4)}<div class="col" style="padding:20px;gap:14px">'
            + entry(LEA, 'commented · 4h', 'Ready for review: this is the flicker from <span style="color:var(--accent)">#117</span>.')
            + entry(KENJI, 'approved · 3h') + thread
            + entry(KENJI, 'commented · 2h', 'Approving. One question about the heap, inline.') + '</div></aside>')

def not_connected_panel():
    return (f'<aside class="insp" style="height:300px"><div class="bar" style="padding:0 12px 0 16px"><div class="tabs"><span class="tab">Message</span><span class="tab on">Files</span></div>'
            f'<div class="grow"></div>{btn("", "copy", "ghost")}</div><div class="col" style="padding:20px;gap:18px">'
            f'<div class="col" style="gap:8px"><span class="label">Commit</span><span class="title">{PR_TITLE}</span></div>'
            f'<div class="kv"><span class="k">Author</span><span class="row" style="gap:8px">{avatar("L", 4)}<span style="font-weight:500">Lea Brandt</span></span>'
            f'<span class="k">Commit</span><span class="row"><span class="chip">1cabd07c</span></span>'
            f'<span class="k">Pull request</span><span class="faint" style="white-space:nowrap;overflow:hidden;text-overflow:ellipsis">Connect GitHub to see pull requests.</span></div>'
            f'<p class="muted" style="margin:0;font-size:12px">With no token (and no sign-in yet), the row says so instead of a pull request. '
            f'GitLab and Bitbucket say "Pull requests on GitLab are coming next."</p></div></aside>')

def merge_panels():
    """The merge box once nothing is in the way, then confirming the squash commit's message."""
    facts = (f'<div class="row" style="gap:10px;align-items:flex-start"><span class="ok" style="margin-top:1px">{icon("check", 16)}</span>'
             f'<div class="col grow"><span style="font-weight:600">All checks passed</span><span class="muted" style="font-size:12px">2 checks</span></div></div>'
             f'<div class="row" style="gap:10px;align-items:flex-start;margin-top:10px"><span class="ok" style="margin-top:1px">{icon("check", 16)}</span>'
             f'<div class="col grow"><span style="font-weight:600">No conflicts with main</span><span class="muted" style="font-size:12px">No approvals yet · ready to merge</span></div></div>')
    ready = (f'<div class="card" style="padding:14px 14px 12px;width:320px">{facts}'
             f'<div class="row" style="margin-top:12px">{btn("Squash and merge", "merge", "primary")}{btn("", "chevron", "secondary", extra="disabled")}</div></div>')
    confirm = (f'<div class="card" style="padding:14px 14px 12px;width:320px">{facts}'
               f'<div class="composer" style="margin-top:12px;min-height:0;padding:5px 10px;color:var(--ink)">Shorter zsh prompt (#12)</div>'
               f'<div class="row" style="margin-top:10px">{btn("Confirm squash and merge", "merge", "primary")}{btn("Cancel", None, "ghost")}</div></div>')
    return f'<div class="row" style="gap:24px;align-items:flex-start">{ready}{confirm}</div>'

def commit_filter_panel():
    """The cockpit's Commits with one commit picked: its id in the accent, and the way back to all."""
    return (f'<aside class="side" style="height:auto;padding-bottom:14px">'
            f'<div class="row" style="padding:16px 16px 8px;justify-content:space-between"><span class="label">Commits</span>'
            f'<span class="monosm" style="color:var(--accent)">8f566f4 · show all</span></div>'
            f'<div class="col" style="padding:0 16px;gap:6px;font-size:12px"><div class="row"><span class="monosm" style="color:var(--accent)">8f566f4</span><span>Measure avatar cache…</span></div>'
            f'<div class="row"><span class="monosm faint">1cabd07</span><span class="muted">Evict avatars by last…</span></div></div></aside>')

def page_extras():
    panel = lambda title, body, w=740: f'<div class="col" style="gap:8px;width:{w}px"><span class="label">{title}</span>{body}</div>'
    return (BASE.replace('width:1552px;', 'width:1552px;') +
            f'<div class="row" style="gap:24px;align-items:flex-start;flex-wrap:wrap">'
            f'{panel("Draft comments, a pending reply, the composer", drafts_panel())}'
            f'<div class="col" style="gap:24px">{panel("Split", split_panel())}{panel("Viewed, then ticked: the file moves to the Viewed shelf (shown open)", viewed_panel())}</div>'
            f'{panel("Inspector: Checks", checks_panel(), 361)}{panel("Inspector: Conversation", conversation_panel(), 361)}'
            f'{panel("Inspector: provider not connected", not_connected_panel(), 361)}'
            f'{panel("Merge box: ready, then confirming the squash commit", merge_panels())}'
            f'{panel("Cockpit: one commit picked", commit_filter_panel(), 248)}</div>' + END)

if __name__ == '__main__':
    open(f'{OUT}/extras.html', 'w').write(page_extras())
    print('ok')

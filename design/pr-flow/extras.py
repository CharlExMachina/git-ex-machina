"""States the C + B flow needs that its frames don't draw, in the same parts: drafts in the
review cockpit, Split, Viewed ticked, the inspector's Checks and Conversation tabs, and a
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
    return f'<div class="cardc" style="overflow:hidden">{head(False)}{head(True)}</div>'

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

def page_extras():
    panel = lambda title, body, w=740: f'<div class="col" style="gap:8px;width:{w}px"><span class="label">{title}</span>{body}</div>'
    return (BASE.replace('width:1552px;', 'width:1552px;') +
            f'<div class="row" style="gap:24px;align-items:flex-start;flex-wrap:wrap">'
            f'{panel("Draft comments, a pending reply, the composer", drafts_panel())}'
            f'<div class="col" style="gap:24px">{panel("Split", split_panel())}{panel("Viewed, then ticked", viewed_panel())}</div>'
            f'{panel("Inspector: Checks", checks_panel(), 361)}{panel("Inspector: Conversation", conversation_panel(), 361)}'
            f'{panel("Inspector: provider not connected", not_connected_panel(), 361)}</div>' + END)

if __name__ == '__main__':
    open(f'{OUT}/extras.html', 'w').write(page_extras())
    print('ok')

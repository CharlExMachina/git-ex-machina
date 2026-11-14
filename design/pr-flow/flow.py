"""The C + B flow as a storyboard: commit → PR on the graph → review cockpit → back, approved."""
from pages import *
import pages

ROWS = [
    ('wt', '', 'Uncommitted changes', '', '', '', 1),
    ('merge', ref('head', 'main') + ref('remote', 'origin/main', 1), 'Merge feature/graph-virtualization', 'A', '2h', '5832000', 1),
    ('c', ref('branch', 'feature/graph-virtualization', 2), 'Virtualize commit rows past 10k', 'K', '3h', 'c0e8db2', 2),
    ('c', ref('branch', 'fix/avatar-cache', 4), PR_TITLE, 'L', '5h', '1cabd07', 4),
    ('c', '', 'Spring the sidebar collapse', 'A', 'Yesterday', 'bd3b72a', 1),
    ('c', '', 'Draw lanes with cubic joins', 'K', 'Mon', '4f7549e', 2),
    ('c', '', 'Measure avatar cache pressure', 'L', 'Mon', '8f566f4', 4),
    ('c', ref('tag', 'v0.4.0'), 'Fix avatar cache eviction', 'L', 'Mon', '76245e5', 1),
    ('c', '', 'Start graph virtualization', 'K', 'Nov 8', '9c902d7', 2),
] + [('c', '', s, w, d, h, 1) for s, w, d, h in [
    ('Bump rune to 0.9', 'A', 'Nov 7', '5b906db'), ('Tune brass for the light theme', 'K', 'Nov 6', '4166440'),
    ('Split view remembers column widths', 'A', 'Nov 5', '6572a47'), ('Read refs with gix instead of shelling out', 'L', 'Nov 4', '8c1088a'),
    ('Window chrome and traffic-light inset', 'A', 'Nov 3', '8e104fc'), ('Persist pinned repositories', 'K', 'Nov 2', 'c3d1863'),
    ('Watch .git for external changes', 'L', 'Nov 2', '50311b1'), ('Group repositories by owner', 'A', 'Nov 1', '40e248d'),
    ('Diff view wraps at 100 columns', 'K', 'Oct 31', '91232c0'), ('Load history in pages of 500 behind the scenes', 'L', 'Oct 30', 'b13bbf7'),
    ('Ahead and behind counts per repository', 'A', 'Oct 29', '24647fd'), ('Cache lane layout between fetches', 'K', 'Oct 29', '36627ee'),
    ('Recessed search field in the sidebar', 'L', 'Oct 28', 'c866fc8')]]
NAMES = {'A': ('A', 3, 'Ana Ruiz'), 'K': ('K', 2, 'Kenji Mori'), 'L': ('L', 4, 'Lea Brandt')}
X = {1: 20, 2: 34, 4: 48}

def history(pr_mode, selected=None, approvals=1):
    """The history list; in `pr_mode` the PR's lane is lit, the rest dimmed, with a PR row above its tip."""
    banner_at = 3 if pr_mode else None
    def cy(i):
        return 15 + 30 * i + (34 if banner_at is not None and i >= banner_at else 0)
    out, dim_g, pr_g = '', [], []
    n = len(ROWS)
    dim_g.append(f'<path d="M20 {cy(1)}V{cy(n) + 30}" stroke="var(--lane-1)"/>')
    dim_g.append(f'<path d="M20 {cy(1)}C20 {cy(1)+9} 34 {cy(1)+5} 34 {cy(1)+15}V{cy(8)}" stroke="var(--lane-2)"/>')
    dim_g.append(f'<path d="M34 {cy(8)}C34 {cy(8)+9} 20 {cy(8)+5} 20 {cy(9)}" stroke="var(--lane-2)"/>')
    pr_g.append(f'<path d="M48 {cy(3)}V{cy(6)}C48 {cy(6)+9} 20 {cy(6)+5} 20 {cy(7)}" stroke="var(--lane-4)"/>')
    dim_g.append(f'<circle cx="20" cy="{cy(0)}" r="4.5" fill="none" stroke="var(--accent)" stroke-width="1.5" stroke-dasharray="2.5 2"/>')
    for i, (kind, refs, subj, who, date, sha, lane) in enumerate(ROWS):
        if i == banner_at:
            appr = f'{approvals} approval{"s" if approvals != 1 else ""}'
            out += (f'<div class="row" style="height:30px;padding:0 12px 0 60px;margin:2px 0;border-radius:6px;background:color-mix(in srgb,var(--lane-4) 12%,transparent);box-shadow:inset 0 0 0 1px color-mix(in srgb,var(--lane-4) 40%,transparent)">'
                    f'<span style="color:var(--lane-4)">{icon("pr", 14)}</span><span style="font-weight:600">#131</span><span class="muted">2 commits · Lea Brandt{" · review requested from you" if approvals == 1 else " · you approved"}</span>'
                    f'<span class="grow"></span><span class="bad row" style="gap:4px;font-size:12px">{icon("x", 12)}1 failing</span><span class="ok row" style="gap:4px;font-size:12px;margin-left:10px">{icon("check", 12)}{appr}</span></div>')
        if i > 0:
            ground = 'var(--raised)' if sha == selected else 'var(--canvas)'
            dot = f'<circle cx="{X[lane]}" cy="{cy(i)}" r="6.5" fill="{ground}"/>'
            if kind == 'merge':
                dot += f'<circle cx="20" cy="{cy(i)}" r="4.5" stroke="var(--lane-1)" stroke-width="2"/><circle cx="20" cy="{cy(i)}" r="1.75" fill="var(--lane-1)"/>'
            else:
                dot += f'<circle cx="{X[lane]}" cy="{cy(i)}" r="4.5" fill="var(--lane-{lane})"/>'
            (pr_g if pr_mode and lane == 4 else dim_g).append(dot)
        dim = pr_mode and lane != 4
        sel = sha and sha == selected
        style = 'background:var(--raised);box-shadow:var(--shadow-lifted);border-radius:6px;' if sel else ''
        if kind == 'wt':
            out += (f'<div class="row {"dim" if dim else ""}" style="height:30px;gap:6px;padding-right:12px"><span style="width:52px"></span>'
                    f'<span class="grow" style="font-weight:500">Uncommitted changes</span><span class="gu-count">3</span></div>')
            continue
        merge = kind == 'merge'
        av = avatar(*NAMES[who][:2])
        out += (f'<div class="row {"dim" if dim else ""}" style="height:30px;padding-right:12px;gap:0;{style}">'
                f'<span style="width:60px;flex:none"></span><span class="row grow" style="gap:6px">{refs}<span style="font-weight:{400 if merge else 500};{"color:var(--ink-muted)" if merge else ""}">{subj}</span></span>'
                f'<span class="row" style="width:128px;gap:6px;font-size:12px;color:var(--ink-muted)">{av}{NAMES[who][2]}</span>'
                f'<span style="width:64px;text-align:right;font-size:12px;color:var(--ink-muted)">{date}</span>'
                f'<span style="width:60px;text-align:right" class="monosm {"muted" if sel else "faint"}">{sha}</span></div>')
    svg = (f'<svg width="60" height="760" style="position:absolute;left:8px;top:6px" fill="none" stroke-width="2" stroke-linecap="round">'
           f'<g opacity="{".38" if pr_mode else "1"}">{"".join(dim_g)}</g>{"".join(pr_g)}</svg>')
    header = (f'<div class="row" style="height:24px;padding:0 12px 0 58px;margin:0 8px;border-bottom:1px solid var(--line);font-size:11px;color:var(--ink-faint)"><span class="grow">Description</span>'
              f'<span style="width:128px">Author</span><span style="width:64px;text-align:right">Date</span><span style="width:60px;text-align:right">Commit</span></div>')
    return header + f'<div style="position:relative;padding:6px 8px 0;overflow:hidden;flex:1">{svg}{out}</div>'

def callout(n, text, x, y, align='left'):
    side = 'left' if align == 'left' else 'right'
    return (f'<div style="position:absolute;{side}:{x}px;top:{y}px;z-index:5;display:flex;align-items:center;gap:8px;'
            f'background:#7cc0ff;color:#06121f;font:600 13px/18px var(--font-sans);padding:6px 10px 6px 6px;border-radius:999px;'
            f'box-shadow:0 0 0 3px rgba(124,192,255,.25),0 6px 18px rgba(0,0,0,.45)">'
            f'<span style="width:22px;height:22px;border-radius:50%;background:#06121f;color:#7cc0ff;display:grid;place-items:center;font-size:12px">{n}</span>{text}</div>')

def marker(n, text, side='left'):
    """A numbered label beside the element marked `data-target`, with a ring around it, placed
    by script once the page has laid out."""
    return f'''<div id="ring" style="position:absolute;border-radius:8px;box-shadow:0 0 0 2px #7cc0ff,0 0 0 6px rgba(124,192,255,.25);z-index:4"></div>
<div id="callout" style="position:absolute;z-index:5;display:flex;align-items:center;gap:8px;background:#7cc0ff;color:#06121f;font:600 13px/18px var(--font-sans);padding:6px 12px 6px 6px;border-radius:999px;box-shadow:0 0 0 3px rgba(124,192,255,.25),0 6px 18px rgba(0,0,0,.45);white-space:nowrap"><span style="width:22px;height:22px;border-radius:50%;background:#06121f;color:#7cc0ff;display:grid;place-items:center;font-size:12px">{n}</span>{text}</div>
<script>
const t=document.querySelector('[data-target]'),w=document.querySelector('.win'),r=t.getBoundingClientRect(),q=w.getBoundingClientRect();
const ring=document.getElementById('ring'),c=document.getElementById('callout');
ring.style.left=(r.left-q.left-3)+'px';ring.style.top=(r.top-q.top-3)+'px';ring.style.width=(r.width+6)+'px';ring.style.height=(r.height+6)+'px';
if('{side}'==='below'){{c.style.top=(r.bottom-q.top+14)+'px';c.style.left=(r.left-q.left)+'px';}}else{{
c.style.top=(r.top-q.top+r.height/2-c.offsetHeight/2)+'px';
c.style.left=('{side}'==='left'?(r.left-q.left-c.offsetWidth-16):(r.right-q.left+16))+'px';}}
</script>'''

def ring(x, y, w, h):
    return f'<div style="position:absolute;left:{x}px;top:{y}px;width:{w}px;height:{h}px;border-radius:8px;box-shadow:0 0 0 2px #7cc0ff,0 0 0 6px rgba(124,192,255,.25);z-index:4"></div>'

def summary_line():
    return (f'<div class="row" style="padding:10px 16px 10px 24px;border-bottom:1px solid var(--line);font-size:12px;gap:6px;justify-content:space-between">'
            f'<span class="row" style="gap:6px">{icon("branch", 12)}<b style="font-weight:500">main</b><span class="monosm muted">↑4</span><span class="muted">· 1,284 commits · fetched 2 minutes ago</span></span>'
            f'<span class="row" style="gap:12px"><span class="faint" style="font-size:11.5px">⌘-click or ⇧-click to select several</span>'
            f'<label class="gu-search" style="width:220px">{icon("search")}<input placeholder="Search commits"><kbd class="gu-kbd">⌘F</kbd></label></span></div>')

def win(side, main, insp, overlay=''):
    return BASE + f'<div class="win" style="position:relative">{side}{main}{insp}{overlay}</div>' + END

def step1():
    main = (f'<main class="main"><div class="bar">{btn("Checkout")}{btn("Cherry-pick")}{btn("Revert")}{btn("Branch from here", "branch")}</div>'
            f'{summary_line()}{history(False, selected="1cabd07")}</main>')
    insp = inspector_commit('open').replace('style="height:640px"', '')
    insp = insp.replace('gu-btn-sm" >' + icon('pr', 14), 'gu-btn-sm" data-target>' + icon('pr', 14), 1)
    return win(sidebar(), main, insp, marker(1, 'See pull request', 'left'))

def pr_inspector(approved):
    revs = reviewers()
    merge = merge_box()
    if approved:
        revs = revs.replace(f'<span class="wait row" style="gap:4px;font-size:12px">{icon("clock", 12)}Requested</span>',
                            f'<span class="ok row" style="gap:4px;font-size:12px">{icon("check", 12)}Approved</span>')
        merge = merge.replace('1 approval · your review is requested', '2 approvals · ready once checks pass')
    return (f'<aside class="insp"><div class="bar" style="padding:0 12px 0 16px"><div class="tabs"><span class="tab on">Overview</span><span class="tab">Checks</span><span class="tab">Conversation<span class="count">{5 if approved else 4}</span></span></div>'
            f'<div class="grow"></div>{btn("", "external", "ghost")}</div>'
            f'<div class="col" style="padding:20px;gap:18px"><div class="col" style="gap:8px"><span class="label">Pull request</span>{pr_header(compact=True)}</div>'
            f'<div class="muted" style="font-size:12.5px">{DESCRIPTION}</div>{merge}'
            f'<div class="col" style="gap:2px"><div class="label" style="padding:0 8px 6px">Reviewers</div>{revs}</div></div></aside>')

def step2():
    main = (f'<main class="main"><div class="bar">{btn("Review", "eye", "primary", extra="data-target")}{btn("Check out", "branch")}{btn("Squash and merge", "merge", extra="disabled")}'
            f'<div class="grow"></div>{btn("Close pull request", "x", "ghost")}</div>'
            f'<div class="row" style="padding:10px 16px 10px 24px;border-bottom:1px solid var(--line);font-size:12px;gap:6px"><span style="color:var(--lane-4)">{icon("pr", 12)}</span>'
            f'<span style="font-weight:500">Pull request #131</span><span class="muted">· fix/avatar-cache → main · its commits are lit, the rest of the history dims</span></div>'
            f'{history(True)}</main>')
    return win(sidebar(), main, pr_inspector(False), marker(2, 'Review opens the cockpit', 'below'))

def step3():
    html = pages.page_b()
    html = html.replace(f'<div style="padding:14px 16px 10px">{btn("atlas-web", "back", "ghost")}</div>',
                        f'<div style="padding:14px 16px 10px">{btn("Pull request #131", "back", "ghost")}</div>')
    html = html.replace('gu-btn-primary gu-btn-sm" ><span>Submit review', 'gu-btn-primary gu-btn-sm" data-target><span>Submit review', 1)
    return html.replace('</div></div></body>', f'{marker(3, "Submit review", "right")}</div></div></body>').replace('<div class="win">', '<div class="win" style="position:relative">')

def step4():
    main = (f'<main class="main"><div class="bar">{btn("Reviewed", "check", "secondary")}{btn("Check out", "branch")}{btn("Squash and merge", "merge", extra="disabled")}'
            f'<div class="grow"></div>{btn("Close pull request", "x", "ghost")}</div>'
            f'<div class="row" style="padding:10px 16px 10px 24px;border-bottom:1px solid var(--line);font-size:12px;gap:6px"><span style="color:var(--lane-4)">{icon("pr", 12)}</span>'
            f'<span style="font-weight:500">Pull request #131</span><span class="muted">· fix/avatar-cache → main</span>'
            f'<span class="grow"></span><span class="ok row" style="gap:4px">{icon("check", 12)}Your approval was sent to GitHub</span></div>'
            f'{history(True, approvals=2)}</main>')
    return win(sidebar(), main, pr_inspector(True))

if __name__ == '__main__':
    for name, fn in [('flow1', step1), ('flow2', step2), ('flow3', step3), ('flow4', step4)]:
        open(f'{OUT}/{name}.html', 'w').write(fn())
    print('ok')

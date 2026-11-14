"""Shared pieces for the PR designs: full-size app windows built from `design/system/components.css`.

Run `python3 pages.py && python3 flow.py` here to regenerate the HTML; open it in a browser from this folder."""
import sys, os

OUT = os.path.dirname(os.path.abspath(__file__))

ICONS = {
    'branch': '<circle cx="5" cy="3.5" r="1.5"/><circle cx="5" cy="12.5" r="1.5"/><circle cx="11" cy="4.5" r="1.5"/><path d="M5 5v6"/><path d="M11 6c0 3.5-6 2.5-6 5"/>',
    'search': '<circle cx="7" cy="7" r="4.5"/><path d="M10.5 10.5 13.5 13.5"/>',
    'fetch': '<path d="M8 2.5v8"/><path d="M4.5 7 8 10.5 11.5 7"/><path d="M3 13.5h10"/>',
    'push': '<path d="M8 13.5v-8"/><path d="M4.5 9 8 5.5 11.5 9"/><path d="M3 2.5h10"/>',
    'plus': '<path d="M8 3v10M3 8h10"/>',
    'sidebar': '<rect x="2" y="3" width="12" height="10" rx="2"/><path d="M6 3v10"/>',
    'copy': '<rect x="5.5" y="5.5" width="8" height="8" rx="1.5"/><path d="M10.5 5.5v-2a1 1 0 0 0-1-1h-6a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2"/>',
    'chevron': '<path d="M4.5 6.5 8 10l3.5-3.5"/>',
    'tag': '<path d="M2.5 2.5h5l6 6-5 5-6-6z"/><circle cx="5.5" cy="5.5" r="0.75"/>',
    'merge': '<circle cx="4.5" cy="3.5" r="1.5"/><circle cx="4.5" cy="12.5" r="1.5"/><circle cx="11.5" cy="10" r="1.5"/><path d="M4.5 5v6"/><path d="M4.5 5c0 3 3 5 5.5 5"/>',
    'pr': '<circle cx="4.5" cy="3.5" r="1.5"/><circle cx="4.5" cy="12.5" r="1.5"/><circle cx="11.5" cy="12.5" r="1.5"/><path d="M4.5 5v6"/><path d="M11.5 11V6.5a2 2 0 0 0-2-2H7"/><path d="M8.5 3 7 4.5 8.5 6"/>',
    'check': '<path d="M3.5 8.5 6.5 11.5 12.5 4.5"/>',
    'x': '<path d="M4.5 4.5l7 7M11.5 4.5l-7 7"/>',
    'dot': '<circle cx="8" cy="8" r="3"/>',
    'comment': '<path d="M2.5 4a1.5 1.5 0 0 1 1.5-1.5h8A1.5 1.5 0 0 1 13.5 4v5.5A1.5 1.5 0 0 1 12 11H7l-3 2.5V11a1.5 1.5 0 0 1-1.5-1.5z"/>',
    'back': '<path d="M9.5 3.5 5 8l4.5 4.5"/>',
    'external': '<path d="M9 2.5h4.5V7"/><path d="M13.5 2.5 7.5 8.5"/><path d="M11.5 9.5v3a1 1 0 0 1-1 1h-7a1 1 0 0 1-1-1v-7a1 1 0 0 1 1-1h3"/>',
    'eye': '<path d="M1.5 8s2.5-4.5 6.5-4.5S14.5 8 14.5 8s-2.5 4.5-6.5 4.5S1.5 8 1.5 8z"/><circle cx="8" cy="8" r="2"/>',
    'clock': '<circle cx="8" cy="8" r="5.5"/><path d="M8 5v3l2 1.5"/>',
    'issue': '<circle cx="8" cy="8" r="5.5"/><circle cx="8" cy="8" r="1"/>',
    'commit': '<circle cx="8" cy="8" r="2.5"/><path d="M1.5 8h4M10.5 8h4"/>',
    'file': '<path d="M4 1.5h5l3.5 3.5v8.5a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-11a1 1 0 0 1 1-1z"/><path d="M9 1.5V5h3.5"/>',
}

def icon(name, size=14, style=''):
    return (f'<svg class="gu-icon" width="{size}" height="{size}" viewBox="0 0 16 16" fill="none" stroke="currentColor" '
            f'stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round" style="{style}">{ICONS[name]}</svg>')

BASE = f'''<!doctype html><html><head><meta charset="utf-8"><style>
@font-face{{font-family:"Hanken Grotesk";src:url("../../crates/gxm-app/assets/fonts/HankenGrotesk-Variable.ttf");font-weight:100 900}}
@font-face{{font-family:"Geist Mono";src:url("../../crates/gxm-app/assets/fonts/GeistMono-Variable.ttf");font-weight:100 900}}
</style><link rel="stylesheet" href="components.css"><style>
html,body{{margin:0}}
.page{{padding:24px;background:var(--well);width:1552px;font-family:var(--font-sans);font-size:13px;line-height:18px;color:var(--ink);-webkit-font-smoothing:antialiased}}
.win{{display:flex;height:852px;width:1552px;background:var(--chrome);border-radius:14px;overflow:hidden;box-shadow:0 0 0 1px var(--line),0 24px 64px rgba(0,0,0,.32)}}
.side{{width:248px;flex:none;display:flex;flex-direction:column;border-right:1px solid var(--line);background:var(--chrome)}}
.main{{flex:1;min-width:0;display:flex;flex-direction:column;background:var(--canvas)}}
.insp{{width:360px;flex:none;display:flex;flex-direction:column;background:var(--raised);border-left:1px solid var(--line);min-height:0}}
.bar{{height:48px;flex:none;display:flex;align-items:center;gap:8px;padding:0 16px;border-bottom:1px solid var(--line);background:var(--chrome)}}
.lights{{display:flex;gap:8px}} .lights i{{width:12px;height:12px;border-radius:50%;display:block}}
.label{{font-size:11px;line-height:14px;font-weight:600;letter-spacing:.06em;text-transform:uppercase;color:var(--ink-muted)}}
.muted{{color:var(--ink-muted)}} .faint{{color:var(--ink-faint)}} .mono{{font-family:var(--font-mono);font-size:12px}}
.monosm{{font-family:var(--font-mono);font-size:11px;line-height:16px;font-weight:500}}
.title{{font-size:18px;line-height:24px;font-weight:600;letter-spacing:-.01em}}
.title-lg{{font-size:22px;line-height:28px;font-weight:600;letter-spacing:-.015em}}
.row{{display:flex;align-items:center;gap:8px}} .grow{{flex:1;min-width:0}} .col{{display:flex;flex-direction:column}}
.card{{background:var(--raised);border-radius:12px;box-shadow:inset 0 0 0 1px var(--line)}}
.cardc{{background:var(--canvas);border-radius:12px;box-shadow:inset 0 0 0 1px var(--line)}}
.chip{{font-family:var(--font-mono);font-size:11px;line-height:16px;font-weight:500;padding:1px 6px;border-radius:4px;background:var(--canvas);box-shadow:inset 0 0 0 1px var(--line);color:var(--ink)}}
.state{{display:inline-flex;align-items:center;gap:5px;height:20px;padding:0 8px 0 6px;border-radius:999px;font-size:11.5px;font-weight:600}}
.st-open{{background:var(--added-soft);color:var(--added)}} .st-merged{{background:color-mix(in srgb,var(--lane-6) 18%,transparent);color:var(--lane-6)}}
.st-fail{{background:var(--removed-soft);color:var(--removed)}}
.ok{{color:var(--added)}} .bad{{color:var(--removed)}} .wait{{color:var(--accent)}}
.kv{{display:grid;grid-template-columns:96px 1fr;row-gap:10px;column-gap:12px;align-items:center;font-size:12px}}
.kv>.k{{color:var(--ink-faint)}}
.md p{{margin:0 0 8px}} .md code{{font-family:var(--font-mono);font-size:11.5px;background:var(--well);padding:1px 4px;border-radius:4px}}
.md ul{{margin:0 0 8px;padding-left:18px}}
.tabs{{display:flex;gap:2px;padding:2px;border-radius:8px;background:var(--well);box-shadow:var(--shadow-well)}}
.tab{{height:24px;padding:0 12px;border-radius:6px;display:inline-flex;align-items:center;gap:6px;font-size:12px;font-weight:500;color:var(--ink-muted)}}
.tab.on{{background:linear-gradient(var(--control),var(--control-bottom));color:var(--ink);box-shadow:var(--shadow-raised)}}
.count{{font-family:var(--font-mono);font-size:10.5px;line-height:14px;padding:0 5px;border-radius:999px;background:var(--control-hover);color:var(--ink-muted)}}
.tl{{position:relative;padding-left:28px}} .tl:before{{content:"";position:absolute;left:11px;top:0;bottom:0;width:2px;background:var(--line)}}
.tli{{position:relative;margin-bottom:14px}} .tli .node{{position:absolute;left:-28px;top:0;width:24px;height:24px;border-radius:50%;display:grid;place-items:center;background:var(--canvas);box-shadow:inset 0 0 0 1px var(--line);color:var(--ink-muted)}}
.composer{{border-radius:8px;background:var(--well);box-shadow:var(--shadow-well);padding:10px 12px;color:var(--ink-faint);min-height:56px}}
.thread{{border-top:1px solid var(--line);padding:10px 12px;background:var(--raised)}}
.note{{position:absolute;font-family:var(--font-sans);font-size:12px;line-height:16px;font-weight:600;color:#1c1307;background:#e2ad62;padding:4px 8px;border-radius:6px;box-shadow:0 2px 8px rgba(0,0,0,.4)}}
.dim{{opacity:.38}}
</style></head><body><div class="page" data-theme="dark">'''

END = '</div></body></html>'

def btn(label, ic=None, variant='secondary', badge=None, sm=True, extra=''):
    cls = f'gu-btn gu-btn-{variant}' + (' gu-btn-sm' if sm else '') + ('' if label else ' gu-btn-icononly')
    inner = (icon(ic, 14 if sm else 16) if ic else '') + (f'<span>{label}</span>' if label else '') + (f'<span class="gu-btn-badge">{badge}</span>' if badge else '')
    return f'<button class="{cls}" {extra}>{inner}</button>'

def avatar(initial, lane, size=16):
    return f'<span class="gu-avatar" style="--lane:var(--lane-{lane});width:{size}px;height:{size}px;font-size:{9 if size<=16 else 10}px">{initial}</span>'

ANA, KENJI, LEA = ('A', 3, 'Ana Ruiz'), ('K', 2, 'Kenji Mori'), ('L', 4, 'Lea Brandt')

def person(p, size=16):
    return f'<span class="row" style="gap:6px">{avatar(p[0], p[1], size)}<span>{p[2]}</span></span>'

def ref(kind, name, lane=1):
    if kind == 'head':
        return f'<span class="gu-ref gu-ref-head" style="--lane:var(--lane-{lane})"><span class="gu-ref-dot"></span>{name}</span>'
    if kind == 'remote':
        a, b = name.split('/', 1)
        return f'<span class="gu-ref gu-ref-remote" style="--lane:var(--lane-{lane})"><span class="gu-ref-dot"></span><span><span class="gu-ref-prefix">{a}/</span>{b}</span></span>'
    if kind == 'tag':
        return f'<span class="gu-ref gu-ref-tag">{icon("tag", 12)}{name}</span>'
    return f'<span class="gu-ref" style="--lane:var(--lane-{lane})"><span class="gu-ref-dot"></span>{name}</span>'

def repo_row(name, owner, branch, lane, side='', sel=False):
    return (f'<div class="gu-repo" aria-selected="{str(sel).lower()}" style="--lane:var(--lane-{lane})"><span class="gu-repo-tile">{name[0].upper()}</span>'
            f'<span class="gu-repo-text"><span class="gu-repo-name">{name}</span><span class="gu-repo-meta">{icon("branch", 11)}{branch} <span>· {owner}</span></span></span>'
            f'<span class="gu-repo-side">{side}</span></div>')

def sidebar():
    lights = '<div class="lights"><i style="background:#ff5f57"></i><i style="background:#febc2e"></i><i style="background:#28c840"></i></div>'
    pinned = repo_row('atlas-web', 'studio', 'main', 1, '<span>↑4</span><span class="gu-count">3</span>', True) + repo_row('rune-playground', 'personal', 'spring-tuning', 2)
    rest = (repo_row('payments-api', 'studio', 'release/2.3', 3, '<span>↓4</span>') + repo_row('brand-tokens', 'studio', 'main', 4, '<span class="gu-count">12</span>')
            + repo_row('dotfiles', 'personal', 'main', 5) + repo_row('field-notes', 'personal', 'drafts', 6) + repo_row('infra-terraform', 'studio', 'main', 1, '<span>↓1</span>'))
    return (f'<aside class="side"><div class="bar" style="padding:0 12px;background:var(--chrome)">{lights}<div class="grow"></div>{btn("", "sidebar", "ghost")}{btn("", "plus", "ghost")}</div>'
            f'<div style="padding:12px 8px 0"><label class="gu-search">{icon("search")}<input placeholder="Filter repositories"><kbd class="gu-kbd">⌘K</kbd></label></div>'
            f'<div class="label" style="padding:18px 16px 8px">Pinned</div><div class="col" style="padding:0 8px">{pinned}</div>'
            f'<div class="label" style="padding:18px 16px 8px">Repositories</div><div class="col" style="padding:0 8px">{rest}</div>'
            f'<div style="margin-top:auto;padding:12px;border-top:1px solid var(--line)">{btn("Clone repository", "plus", "ghost")}</div></aside>')

def checks_list(compact=False):
    items = [('ok', 'check', 'lint', '38s'), ('ok', 'check', 'unit tests', '1m 12s'), ('bad', 'x', 'journeys · first_look', '2m 03s'), ('ok', 'check', 'design fidelity', '41s')]
    out = ''
    for cls, ic, name, t in items:
        out += f'<div class="row" style="height:26px;padding:0 8px"><span class="{cls}">{icon(ic, 14)}</span><span class="grow">{name}</span><span class="monosm faint">{t}</span></div>'
    return out

def reviewers():
    return (f'<div class="row" style="height:28px;padding:0 8px">{person(KENJI)}<span class="grow"></span><span class="ok row" style="gap:4px;font-size:12px">{icon("check", 12)}Approved</span></div>'
            f'<div class="row" style="height:28px;padding:0 8px">{person(ANA)}<span class="faint" style="font-size:12px">(you)</span><span class="grow"></span><span class="wait row" style="gap:4px;font-size:12px">{icon("clock", 12)}Requested</span></div>')

def hunk_lines(lines):
    out = ''
    for t, a, b, text in lines:
        cls = {'add': ' gu-hunk-add', 'del': ' gu-hunk-del', 'ctx': ''}[t]
        sign = {'add': '+', 'del': '−', 'ctx': ' '}[t]
        out += f'<div class="gu-hunk-line{cls}"><span class="gu-hunk-no">{a}</span><span class="gu-hunk-no">{b}</span><span class="gu-hunk-sign">{sign}</span><span class="gu-hunk-text">{text}</span></div>'
    return out

EVICT_HUNK = [
    ('ctx', 61, 61, 'while self.bytes &gt; self.budget {'),
    ('del', 62, '', '    let oldest = self.by_insert.pop_front();'),
    ('add', '', 62, '    let oldest = self.by_last_paint.pop_min();'),
    ('add', '', 63, '    self.metrics.evicted(oldest.bytes);'),
    ('ctx', 63, 64, '    self.remove(oldest);'),
    ('ctx', 64, 65, '}'),
]

def file_row(st, path, add, rem, sel=False, extra=''):
    d, n = path.rsplit('/', 1)
    pips = ''
    total = add + rem
    na = round(5 * add / total) if total else 0
    nd = min(5 - na, round(5 * rem / total)) if total else 0
    for i in range(5):
        pips += f'<span class="gu-pip {"add" if i < na else "del" if i < na + nd else ""}"></span>'
    stat = (f'<span class="add">+{add}</span>' if add else '') + (f'<span class="del">−{rem}</span>' if rem else '')
    return (f'<div class="gu-file" aria-selected="{str(sel).lower()}"><span class="gu-status gu-status-{st}">{st}</span>'
            f'<span class="gu-file-path"><span class="gu-file-dir">{d}/</span><span class="gu-file-name">{n}</span></span>{extra}'
            f'<span class="gu-file-stat">{stat}<span class="gu-pips">{pips}</span></span></div>')

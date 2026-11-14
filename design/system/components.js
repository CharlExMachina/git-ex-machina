/* @ds-bundle: {"format":4,"namespace":"GitUI","components":[{"name":"Button"},{"name":"SegmentedControl"},{"name":"SearchField"},{"name":"RefBadge"},{"name":"RepoRow"},{"name":"CommitGraphRow"},{"name":"FileChangeRow"},{"name":"DiffHunk"},{"name":"Icon"},{"name":"AppShell"}]} */
(function () {
  var R = window.React, h = R.createElement;
  function cx() { return Array.prototype.filter.call(arguments, Boolean).join(' '); }
  function lane(n) { var i = ((Number(n) || 1) - 1) % 6 + 1; return 'var(--lane-' + i + ')'; }

  /* 16px grid, 1.5 stroke, round caps and joins */
  var P = {
    branch: [['circle', { cx: 5, cy: 3.5, r: 1.5 }], ['circle', { cx: 5, cy: 12.5, r: 1.5 }], ['circle', { cx: 11, cy: 4.5, r: 1.5 }], ['path', { d: 'M5 5v6' }], ['path', { d: 'M11 6c0 3.5-6 2.5-6 5' }]],
    search: [['circle', { cx: 7, cy: 7, r: 4.5 }], ['path', { d: 'M10.5 10.5 13.5 13.5' }]],
    fetch: [['path', { d: 'M8 2.5v8' }], ['path', { d: 'M4.5 7 8 10.5 11.5 7' }], ['path', { d: 'M3 13.5h10' }]],
    pull: [['path', { d: 'M8 2.5v8' }], ['path', { d: 'M4.5 7 8 10.5 11.5 7' }], ['path', { d: 'M3 13.5h10' }]],
    push: [['path', { d: 'M8 13.5v-8' }], ['path', { d: 'M4.5 9 8 5.5 11.5 9' }], ['path', { d: 'M3 2.5h10' }]],
    commit: [['circle', { cx: 8, cy: 8, r: 2.5 }], ['path', { d: 'M1.5 8h4M10.5 8h4' }]],
    merge: [['circle', { cx: 4.5, cy: 3.5, r: 1.5 }], ['circle', { cx: 4.5, cy: 12.5, r: 1.5 }], ['circle', { cx: 11.5, cy: 10, r: 1.5 }], ['path', { d: 'M4.5 5v6' }], ['path', { d: 'M4.5 5c0 3 3 5 5.5 5' }]],
    tag: [['path', { d: 'M2.5 2.5h5l6 6-5 5-6-6z' }], ['circle', { cx: 5.5, cy: 5.5, r: 0.75 }]],
    repo: [['path', { d: 'M12.5 2.5h-8A1.5 1.5 0 0 0 3 4v8.5A1.5 1.5 0 0 1 4.5 11h8z' }], ['path', { d: 'M3 12.5A1.5 1.5 0 0 0 4.5 14h8v-3' }]],
    plus: [['path', { d: 'M8 3v10M3 8h10' }]],
    chevron: [['path', { d: 'M4.5 6.5 8 10l3.5-3.5' }]],
    stash: [['path', { d: 'M2.5 5.5h11v7a1 1 0 0 1-1 1h-9a1 1 0 0 1-1-1z' }], ['path', { d: 'M2.5 5.5 4 2.5h8l1.5 3' }], ['path', { d: 'M6 8.5h4' }]],
    sidebar: [['rect', { x: 2, y: 3, width: 12, height: 10, rx: 2 }], ['path', { d: 'M6 3v10' }]],
    copy: [['rect', { x: 5.5, y: 5.5, width: 8, height: 8, rx: 1.5 }], ['path', { d: 'M10.5 5.5v-2a1 1 0 0 0-1-1h-6a1 1 0 0 0-1 1v6a1 1 0 0 0 1 1h2' }]]
  };

  function Icon(p) {
    var size = p.size || 16, parts = P[p.name] || [];
    return h('svg', { className: cx('gu-icon', p.className), width: size, height: size, viewBox: '0 0 16 16', fill: 'none', stroke: 'currentColor', strokeWidth: 1.5, strokeLinecap: 'round', strokeLinejoin: 'round', 'aria-hidden': true },
      parts.map(function (x, i) { return h(x[0], Object.assign({ key: i }, x[1])); }));
  }

  function Button(p) {
    var variant = p.variant || 'secondary', size = p.size || 'md';
    var rest = Object.assign({}, p); ['variant', 'size', 'icon', 'badge', 'children', 'className', 'pressed'].forEach(function (k) { delete rest[k]; });
    var only = p.icon && (p.children == null || p.children === '');
    return h('button', Object.assign({ type: 'button' }, rest, {
      className: cx('gu-btn', 'gu-btn-' + variant, size === 'sm' && 'gu-btn-sm', only && 'gu-btn-icononly', p.className),
      'data-pressed': p.pressed ? '' : undefined
    }),
      p.icon ? h(Icon, { name: p.icon, size: size === 'sm' ? 14 : 16 }) : null,
      p.children != null && p.children !== '' ? h('span', null, p.children) : null,
      p.badge != null ? h('span', { className: 'gu-btn-badge' }, p.badge) : null);
  }

  function SegmentedControl(p) {
    var opts = p.options || [];
    var st = R.useState(p.defaultValue != null ? p.defaultValue : (opts[0] && opts[0].value));
    var value = p.value != null ? p.value : st[0];
    return h('div', { className: cx('gu-seg', p.className), role: 'tablist', 'aria-label': p.label },
      opts.map(function (o) {
        return h('button', {
          key: o.value, type: 'button', role: 'tab', className: 'gu-seg-opt', 'aria-selected': o.value === value,
          onClick: function () { st[1](o.value); if (p.onChange) p.onChange(o.value); }
        }, o.icon ? h(Icon, { name: o.icon, size: 14 }) : null, o.label);
      }));
  }

  function SearchField(p) {
    return h('label', { className: cx('gu-search', p.className) },
      h(Icon, { name: 'search', size: 14 }),
      h('input', { type: 'search', placeholder: p.placeholder || 'Search', defaultValue: p.defaultValue, value: p.value, onChange: p.onChange, 'aria-label': p.placeholder || 'Search' }),
      p.shortcut ? h('kbd', { className: 'gu-kbd' }, p.shortcut) : null);
  }

  function RefBadge(p) {
    var kind = p.kind || 'branch', name = p.name || '';
    var style = { '--lane': lane(p.lane) };
    if (kind === 'tag') return h('span', { className: 'gu-ref gu-ref-tag', title: 'Tag ' + name }, h(Icon, { name: 'tag', size: 12 }), name);
    if (kind === 'remote') {
      var cut = name.indexOf('/');
      return h('span', { className: 'gu-ref gu-ref-remote', style: style, title: 'Remote branch ' + name },
        h('span', { className: 'gu-ref-dot' }),
        h('span', null, cut > 0 ? h('span', { className: 'gu-ref-prefix' }, name.slice(0, cut + 1)) : null, cut > 0 ? name.slice(cut + 1) : name));
    }
    return h('span', { className: cx('gu-ref', kind === 'head' && 'gu-ref-head'), style: style, title: (kind === 'head' ? 'HEAD → ' : 'Branch ') + name },
      h('span', { className: 'gu-ref-dot' }), name);
  }

  function RepoRow(p) {
    var name = p.name || '';
    return h('div', { className: cx('gu-repo', p.className), role: 'option', 'aria-selected': !!p.selected, onClick: p.onClick, style: { '--lane': lane(p.lane) } },
      h('span', { className: 'gu-repo-tile', 'aria-hidden': true }, name.charAt(0).toUpperCase()),
      h('span', { className: 'gu-repo-text' },
        h('span', { className: 'gu-repo-name' }, name),
        h('span', { className: 'gu-repo-meta' }, h(Icon, { name: 'branch', size: 11 }), p.branch || 'main', p.owner ? h('span', null, '· ' + p.owner) : null)),
      h('span', { className: 'gu-repo-side' },
        p.ahead ? h('span', { title: p.ahead + ' to push' }, '↑' + p.ahead) : null,
        p.behind ? h('span', { title: p.behind + ' to pull' }, '↓' + p.behind) : null,
        p.changes ? h('span', { className: 'gu-count', title: p.changes + ' uncommitted changes' }, p.changes) : null));
  }

  /* graph: { node: col, lane: n, up: bool, down: bool, head: bool,
              rails: [{col, lane}], ins: [{col, lane}], outs: [{col, lane}], cols: n } */
  function CommitGraphRow(p) {
    var g = p.graph || {}, PITCH = 14, PAD = 12, H = 30, MID = 15;
    var cols = g.cols || 1 + Math.max.apply(null, [g.node || 0].concat((g.rails || []).map(function (r) { return r.col; }), (g.ins || []).map(function (r) { return r.col; }), (g.outs || []).map(function (r) { return r.col; })));
    var W = PAD * 2 + (cols - 1) * PITCH, x = function (c) { return PAD + c * PITCH; }, nx = x(g.node || 0);
    var kids = [];
    (g.rails || []).forEach(function (r, i) { kids.push(h('path', { key: 'r' + i, className: 'rail', d: 'M' + x(r.col) + ' 0V' + H, stroke: lane(r.lane) })); });
    (g.ins || []).forEach(function (r, i) { var xc = x(r.col); kids.push(h('path', { key: 'i' + i, className: 'rail', d: 'M' + xc + ' 0C' + xc + ' 10 ' + nx + ' 6 ' + nx + ' ' + MID, stroke: lane(r.lane) })); });
    (g.outs || []).forEach(function (r, i) { var xc = x(r.col); kids.push(h('path', { key: 'o' + i, className: 'rail', d: 'M' + nx + ' ' + MID + 'C' + nx + ' 24 ' + xc + ' 20 ' + xc + ' ' + H, stroke: lane(r.lane) })); });
    if (g.up !== false) kids.push(h('path', { key: 'u', className: 'rail', d: 'M' + nx + ' 0V' + MID, stroke: lane(g.lane) }));
    if (g.down !== false) kids.push(h('path', { key: 'd', className: 'rail', d: 'M' + nx + ' ' + MID + 'V' + H, stroke: lane(g.lane) }));
    kids.push(h('circle', { key: 'ring', className: 'node-ring', cx: nx, cy: MID, r: 6.5 }));
    if (g.head) {
      kids.push(h('circle', { key: 'n', cx: nx, cy: MID, r: 4.5, fill: 'none', stroke: lane(g.lane), strokeWidth: 2 }));
      kids.push(h('circle', { key: 'c', cx: nx, cy: MID, r: 1.75, fill: lane(g.lane) }));
    } else {
      kids.push(h('circle', { key: 'n', cx: nx, cy: MID, r: p.merge ? 3.25 : 4.5, fill: lane(g.lane) }));
    }
    var c = p.commit || {};
    return h('div', { className: cx('gu-commit', p.merge && 'gu-commit-merge', p.className), role: 'row', 'aria-selected': !!p.selected, onClick: p.onClick },
      h('svg', { className: 'gu-commit-graph', width: W, height: H, viewBox: '0 0 ' + W + ' ' + H, 'aria-hidden': true }, kids),
      h('div', { className: 'gu-commit-body' },
        (p.refs || []).map(function (r, i) { return h(RefBadge, Object.assign({ key: i }, r)); }),
        h('span', { className: 'gu-commit-subject' }, c.subject)),
      p.compact ? null : h('div', { className: 'gu-commit-author', style: { '--lane': lane(c.authorLane || 3) } },
        h('span', { className: 'gu-avatar', 'aria-hidden': true }, (c.author || '?').charAt(0)), c.author),
      h('div', { className: 'gu-commit-date' }, c.date),
      p.compact ? null : h('div', { className: 'gu-commit-sha' }, (c.sha || '').slice(0, 7)));
  }

  function FileChangeRow(p) {
    var path = p.path || '', cut = path.lastIndexOf('/');
    var add = p.additions || 0, del = p.deletions || 0, total = add + del, pips = [];
    var na = total ? Math.round(5 * add / total) : 0, nd = total ? Math.min(5 - na, Math.round(5 * del / total)) : 0;
    for (var i = 0; i < 5; i++) pips.push(h('span', { key: i, className: cx('gu-pip', i < na ? 'add' : i < na + nd ? 'del' : '') }));
    var status = p.status || 'M';
    var word = { A: 'Added', M: 'Modified', D: 'Deleted', R: 'Renamed' }[status];
    return h('div', { className: cx('gu-file', p.className), role: 'option', 'aria-selected': !!p.selected, onClick: p.onClick },
      h('span', { className: 'gu-status gu-status-' + status, title: word, 'aria-label': word }, status),
      h('span', { className: 'gu-file-path', title: path },
        cut >= 0 ? h('span', { className: 'gu-file-dir' }, path.slice(0, cut + 1)) : null,
        h('span', { className: 'gu-file-name' }, path.slice(cut + 1))),
      h('span', { className: 'gu-file-stat' },
        add ? h('span', { className: 'add' }, '+' + add) : null,
        del ? h('span', { className: 'del' }, '−' + del) : null,
        h('span', { className: 'gu-pips', 'aria-hidden': true }, pips)));
  }

  function DiffHunk(p) {
    var sign = { add: '+', del: '−', ctx: ' ' };
    return h('div', { className: cx('gu-hunk', p.className) },
      p.header ? h('div', { className: 'gu-hunk-head' }, p.header) : null,
      (p.lines || []).map(function (l, i) {
        var t = l.t || 'ctx';
        return h('div', { key: i, className: cx('gu-hunk-line', t !== 'ctx' && 'gu-hunk-' + t) },
          h('span', { className: 'gu-hunk-no' }, l.a == null ? '' : l.a),
          h('span', { className: 'gu-hunk-no' }, l.b == null ? '' : l.b),
          h('span', { className: 'gu-hunk-sign' }, sign[t]),
          h('span', { className: 'gu-hunk-text' }, l.text));
      }));
  }

  function AppShell(p) {
    return h('div', { className: cx('gu-shell', p.className), style: p.style },
      h('div', { className: 'gu-shell-bar' },
        h('div', null, h('span', { className: 'gu-traffic', 'aria-hidden': true }, h('i'), h('i'), h('i')), p.sidebarBar),
        h('div', null, p.mainBar),
        h('div', null, p.inspectorBar)),
      h('aside', { className: 'gu-shell-side', 'aria-label': 'Repositories' }, p.sidebar),
      h('main', { className: 'gu-shell-main', 'aria-label': 'History' }, p.main),
      h('aside', { className: 'gu-shell-inspector', 'aria-label': 'Inspector' }, p.inspector));
  }

  var api = { Button: Button, SegmentedControl: SegmentedControl, SearchField: SearchField, RefBadge: RefBadge, RepoRow: RepoRow, CommitGraphRow: CommitGraphRow, FileChangeRow: FileChangeRow, DiffHunk: DiffHunk, Icon: Icon, AppShell: AppShell };
  window.GitUI = Object.assign(window.GitUI || {}, api);
})();

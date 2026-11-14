class Component extends DCLogic {
  constructor(props) {
    super(props);
    this.state = this.preset(props.selection ?? 'nothing');
  }
  preset(name) {
    var P = {
      'nothing': { sel: [], anchor: null, changes: false, staged: false },
      'uncommitted': { sel: [], anchor: null, changes: true, staged: false },
      'staged': { sel: [], anchor: null, changes: true, staged: true },
      'one commit': { sel: [1], anchor: 1, changes: false, staged: false },
      'two to squash': { sel: [8, 9], anchor: 8, changes: false, staged: false },
      'range to bisect': { sel: [8, 9, 10, 11, 12], anchor: 8, changes: false, staged: false }
    };
    return P[name] || P['nothing'];
  }
  componentDidUpdate(prev) {
    if (prev.selection !== this.props.selection) this.setState(this.preset(this.props.selection ?? 'nothing'));
  }
  renderVals() {
    var self = this;
    var st = this.state;
    var theme = this.props.theme ?? 'dark';
    var FILES = {
      virt: {
        files: [
          { status: 'M', path: 'src/graph/rows.rs', additions: 84, deletions: 21 },
          { status: 'A', path: 'src/graph/virtual_list.rs', additions: 142 },
          { status: 'M', path: 'src/sidebar.rs', additions: 6, deletions: 9 },
          { status: 'D', path: 'src/graph/legacy_layout.rs', deletions: 63 }
        ],
        header: '@@ -41,6 +41,8 @@ fn layout_row(row: &Row)',
        diff: [
          { a: 41, b: 41, text: 'for lane in lanes.active() {' },
          { t: 'del', a: 42, text: '    draw_line(lane.x, y, lane.x, y + H);' },
          { t: 'add', b: 42, text: '    let x = PAD + lane.col as f32 * PITCH;' },
          { t: 'add', b: 43, text: '    draw_rail(x, y, H, lane.color);' },
          { a: 43, b: 44, text: '}' }
        ]
      },
      ui: {
        files: [
          { status: 'M', path: 'src/sidebar.rs', additions: 38, deletions: 12 },
          { status: 'M', path: 'src/theme/motion.rs', additions: 9, deletions: 2 },
          { status: 'R', path: 'src/theme/brass.json', additions: 3, deletions: 3 }
        ],
        header: '@@ -18,5 +18,7 @@ fn collapse(&mut self)',
        diff: [
          { a: 18, b: 18, text: 'let target = if open { WIDTH } else { 0.0 };' },
          { t: 'del', a: 19, text: 'self.width = target;' },
          { t: 'add', b: 19, text: 'self.width.animate_to(target)' },
          { t: 'add', b: 20, text: '    .motion(motion::SMOOTH);' },
          { a: 20, b: 21, text: 'cx.request_layout();' }
        ]
      },
      cache: {
        files: [
          { status: 'M', path: 'src/avatars/cache.rs', additions: 27, deletions: 31 },
          { status: 'A', path: 'src/avatars/metrics.rs', additions: 54 }
        ],
        header: '@@ -62,6 +62,6 @@ fn evict(&mut self)',
        diff: [
          { a: 62, b: 62, text: 'while self.bytes > self.budget {' },
          { t: 'del', a: 63, text: '    let oldest = self.by_insert.pop_front();' },
          { t: 'add', b: 63, text: '    let oldest = self.by_last_paint.pop_min();' },
          { a: 64, b: 64, text: '    self.remove(oldest);' },
          { a: 65, b: 65, text: '}' }
        ]
      },
      work: {
        files: [
          { status: 'A', path: 'src/toolbar/context.rs', additions: 58 },
          { status: 'M', path: 'src/graph/selection.rs', additions: 12, deletions: 4 },
          { status: 'M', path: 'src/theme/brass.json', additions: 2, deletions: 2 }
        ],
        header: '@@ -0,0 +1,6 @@ src/toolbar/context.rs',
        diff: [
          { t: 'add', b: 1, text: 'pub enum Context {' },
          { t: 'add', b: 2, text: '    Idle, Changes, Staged,' },
          { t: 'add', b: 3, text: '    Commit(Oid),' },
          { t: 'add', b: 4, text: '    Range { oids: Vec<Oid>, linear: bool },' },
          { t: 'add', b: 5, text: '}' }
        ]
      }
    };
    var C = [
      { merge: true, refs: [{ kind: 'head', name: 'main', lane: 1 }, { kind: 'remote', name: 'origin/main', lane: 1 }],
        graph: { cols: 3, node: 0, lane: 1, up: false, head: true, outs: [{ col: 1, lane: 2 }] },
        commit: { subject: 'Merge feature/graph-virtualization', author: 'Ana Ruiz', authorLane: 3, date: '2h', sha: 'a3f9c12e' },
        when: '2 hours ago', parent: '19c4e7f · 7be01d4', set: 'virt', body: 'Brings row virtualization into main. No conflicts.' },
      { refs: [{ kind: 'branch', name: 'feature/graph-virtualization', lane: 2 }],
        graph: { cols: 3, node: 1, lane: 2, rails: [{ col: 0, lane: 1 }] },
        commit: { subject: 'Virtualize commit rows past 10k', author: 'Kenji Mori', authorLane: 2, date: '3h', sha: '7be01d4a' },
        when: '3 hours ago', parent: 'e2d8a61', set: 'virt', body: 'Rows outside the viewport are no longer laid out. Lane state is carried per row, so scrolling stays smooth on large histories.' },
      { refs: [{ kind: 'branch', name: 'fix/avatar-cache', lane: 4 }],
        graph: { cols: 3, node: 2, lane: 4, up: false, rails: [{ col: 0, lane: 1 }, { col: 1, lane: 2 }] },
        commit: { subject: 'Evict avatars by last paint, not insert', author: 'Lea Brandt', authorLane: 4, date: '5h', sha: '4e7c0a92' },
        when: '5 hours ago', parent: 'b9d2f13', set: 'cache', body: 'Avatars that are on screen stay cached even when they were loaded first.' },
      { graph: { cols: 3, node: 0, lane: 1, rails: [{ col: 1, lane: 2 }, { col: 2, lane: 4 }] },
        commit: { subject: 'Spring the sidebar collapse', author: 'Ana Ruiz', authorLane: 3, date: 'Yesterday', sha: '19c4e7f0' },
        when: 'Yesterday at 18:42', parent: '5a0b3c9', set: 'ui', body: 'The sidebar now collapses on the smooth spring instead of snapping.' },
      { graph: { cols: 3, node: 1, lane: 2, rails: [{ col: 0, lane: 1 }, { col: 2, lane: 4 }] },
        commit: { subject: 'Draw lanes with cubic joins', author: 'Kenji Mori', authorLane: 2, date: 'Mon', sha: 'e2d8a611' },
        when: 'Monday at 11:05', parent: 'c81f2b0', set: 'virt', body: 'Branches peel away from a node with a cubic curve rather than a kink.' },
      { graph: { cols: 3, node: 2, lane: 4, down: false, rails: [{ col: 0, lane: 1 }, { col: 1, lane: 2 }], outs: [{ col: 0, lane: 4 }] },
        commit: { subject: 'Measure avatar cache pressure', author: 'Lea Brandt', authorLane: 4, date: 'Mon', sha: 'b9d2f13c' },
        when: 'Monday at 09:30', parent: '5a0b3c9', set: 'cache', body: 'Adds counters for hits, misses and evictions.' },
      { refs: [{ kind: 'tag', name: 'v0.4.0' }], graph: { cols: 3, node: 0, lane: 1, rails: [{ col: 1, lane: 2 }] },
        commit: { subject: 'Fix avatar cache eviction', author: 'Lea Brandt', authorLane: 4, date: 'Mon', sha: '5a0b3c9d' },
        when: 'Monday at 08:12', parent: '0d6e94a', set: 'cache', body: 'Released as v0.4.0.' },
      { graph: { cols: 3, node: 1, lane: 2, down: false, rails: [{ col: 0, lane: 1 }], outs: [{ col: 0, lane: 2 }] },
        commit: { subject: 'Start graph virtualization', author: 'Kenji Mori', authorLane: 2, date: 'Nov 8', sha: 'c81f2b07' },
        when: 'Nov 8 at 16:20', parent: '0d6e94a', set: 'virt', body: 'Scaffolding for the virtual list.' },
      { graph: { cols: 3, node: 0, lane: 1 },
        commit: { subject: 'Bump rune to 0.9', author: 'Ana Ruiz', authorLane: 3, date: 'Nov 7', sha: '0d6e94ab' },
        when: 'Nov 7 at 10:01', parent: '82aa51e', set: 'ui', body: 'Picks up the new spring API.' },
      { graph: { cols: 3, node: 0, lane: 1 },
        commit: { subject: 'Tune brass for the light theme', author: 'Kenji Mori', authorLane: 2, date: 'Nov 6', sha: '82aa51e3' },
        when: 'Nov 6 at 14:48', parent: '3c09df7', set: 'ui', body: 'Darkens accent and lanes so text and marks pass contrast on paper.' },
      { graph: { cols: 3, node: 0, lane: 1 },
        commit: { subject: 'Split view remembers column widths', author: 'Ana Ruiz', authorLane: 3, date: 'Nov 5', sha: '3c09df7b' },
        when: 'Nov 5 at 12:30', parent: 'f61b2e8', set: 'ui', body: 'Column widths persist per window.' },
      { graph: { cols: 3, node: 0, lane: 1 },
        commit: { subject: 'Read refs with gix instead of shelling out', author: 'Lea Brandt', authorLane: 4, date: 'Nov 4', sha: 'f61b2e80' },
        when: 'Nov 4 at 17:15', parent: '9e4471a', set: 'cache', body: 'Ref listing no longer spawns git.' },
      { graph: { cols: 3, node: 0, lane: 1 },
        commit: { subject: 'Window chrome and traffic-light inset', author: 'Ana Ruiz', authorLane: 3, date: 'Nov 3', sha: '9e4471aa' },
        when: 'Nov 3 at 09:44', parent: '6b1d03e', set: 'ui', body: 'Toolbar sits level with the traffic lights.' },
      { graph: { cols: 3, node: 0, lane: 1 },
        commit: { subject: 'Persist pinned repositories', author: 'Kenji Mori', authorLane: 2, date: 'Nov 2', sha: '6b1d03e5' },
        when: 'Nov 2 at 15:10', parent: 'd40e7c2', set: 'ui', body: 'Pinned repositories survive a relaunch.' },
      { graph: { cols: 3, node: 0, lane: 1 },
        commit: { subject: 'Watch .git for external changes', author: 'Lea Brandt', authorLane: 4, date: 'Nov 2', sha: 'd40e7c21' },
        when: 'Nov 2 at 11:02', parent: '71ac9f4', set: 'cache', body: 'Commits made in a terminal show up without a refresh.' },
      { graph: { cols: 3, node: 0, lane: 1 },
        commit: { subject: 'Group repositories by owner', author: 'Ana Ruiz', authorLane: 3, date: 'Nov 1', sha: '71ac9f48' },
        when: 'Nov 1 at 17:36', parent: 'aa03b5d', set: 'ui', body: 'Owner names now sit beside the branch in each row.' },
      { graph: { cols: 3, node: 0, lane: 1 },
        commit: { subject: 'Diff view wraps at 100 columns', author: 'Kenji Mori', authorLane: 2, date: 'Oct 31', sha: 'aa03b5d9' },
        when: 'Oct 31 at 10:20', parent: '2f8c6e1', set: 'virt', body: 'Long lines wrap in the full-width diff view.' },
      { graph: { cols: 3, node: 0, lane: 1 },
        commit: { subject: 'Load history in pages of 500 behind the scenes', author: 'Lea Brandt', authorLane: 4, date: 'Oct 30', sha: '2f8c6e17' },
        when: 'Oct 30 at 09:05', parent: '58e1a0c', set: 'virt', body: 'Older commits stream in as you scroll.' },
      { graph: { cols: 3, node: 0, lane: 1 },
        commit: { subject: 'Ahead and behind counts per repository', author: 'Ana Ruiz', authorLane: 3, date: 'Oct 29', sha: '58e1a0c3' },
        when: 'Oct 29 at 16:44', parent: 'c7d29b4', set: 'ui', body: 'Each repository row shows commits to push and pull.' },
      { graph: { cols: 3, node: 0, lane: 1 },
        commit: { subject: 'Cache lane layout between fetches', author: 'Kenji Mori', authorLane: 2, date: 'Oct 29', sha: 'c7d29b40' },
        when: 'Oct 29 at 10:15', parent: '1e6f8a2', set: 'virt', body: 'Unchanged history keeps its lane layout after a fetch.' },
      { graph: { cols: 3, node: 0, lane: 1 },
        commit: { subject: 'Recessed search field in the sidebar', author: 'Lea Brandt', authorLane: 4, date: 'Oct 28', sha: '1e6f8a29' },
        when: 'Oct 28 at 13:30', parent: '94b0c5e', set: 'ui', body: 'The filter field sits in a well, as the system specifies.' },
      { graph: { cols: 3, node: 0, lane: 1 },
        commit: { subject: 'Open repositories from the command line', author: 'Ana Ruiz', authorLane: 3, date: 'Oct 27', sha: '94b0c5e7' },
        when: 'Oct 27 at 09:58', parent: '3a7e1d0', set: 'cache', body: 'Running the app with a path opens that repository.' },
      { graph: { cols: 3, node: 0, lane: 1 },
        commit: { subject: 'Relative dates in rows, absolute on hover', author: 'Kenji Mori', authorLane: 2, date: 'Oct 26', sha: '3a7e1d06' },
        when: 'Oct 26 at 18:21', parent: 'e05b9f3', set: 'ui', body: 'Rows show 2h or Mon; the inspector shows the full date.' },
      { graph: { cols: 3, node: 0, lane: 1 },
        commit: { subject: 'Read config from the global gitconfig', author: 'Lea Brandt', authorLane: 4, date: 'Oct 25', sha: 'e05b9f31' },
        when: 'Oct 25 at 11:40', parent: '7c2a8d5', set: 'cache', body: 'Author name and email come from your git config.' }
    ];
    var sel = st.sel.slice().sort(function (a, b) { return a - b; });
    var pick = function (i, e) {
      var meta = e && (e.metaKey || e.ctrlKey), shift = e && e.shiftKey;
      var cur = self.state.sel, next, anchor = i;
      if (shift && self.state.anchor != null) {
        var lo = Math.min(self.state.anchor, i), hi = Math.max(self.state.anchor, i);
        next = [];
        for (var k = lo; k <= hi; k++) next.push(k);
        anchor = self.state.anchor;
      } else if (meta) {
        next = cur.indexOf(i) >= 0 ? cur.filter(function (x) { return x !== i; }) : cur.concat([i]);
      } else {
        next = cur.length === 1 && cur[0] === i ? [] : [i];
      }
      self.setState({ sel: next, anchor: anchor, changes: false });
    };
    var commits = C.map(function (c, i) {
      return Object.assign({}, c, { selected: sel.indexOf(i) >= 0, pick: function (e) { pick(i, e); } });
    });

    var n = sel.length;
    var linear = false, noMerge = true;
    if (n >= 2) {
      var L = C[sel[0]].graph.lane;
      var same = sel.every(function (i) { return C[i].graph.lane === L; });
      var seq = [];
      C.forEach(function (c, i) { if (c.graph.lane === L) seq.push(i); });
      var pos = sel.map(function (i) { return seq.indexOf(i); });
      linear = same && pos.every(function (p, k) { return k === 0 || p === pos[k - 1] + 1; });
      noMerge = sel.every(function (i) { return !C[i].merge; });
    }
    var canSquash = n >= 2 && linear && noMerge;
    var canBisect = n >= 3 && linear;
    var mode = st.changes ? (st.staged ? 'staged' : 'changes') : n === 0 ? 'idle' : n === 1 ? 'one' : 'many';

    var sum = function (files) {
      var add = 0, del = 0;
      files.forEach(function (f) { add += f.additions || 0; del += f.deletions || 0; });
      return files.length + ' files · +' + add + ' −' + del;
    };
    var mark = function (files) { return files.map(function (f, k) { return Object.assign({}, f, { selected: k === 0 }); }); };
    var ins;
    if (mode === 'changes' || mode === 'staged') {
      var w = FILES.work;
      ins = { kicker: 'Working tree', title: 'Uncommitted changes', isCommit: false, isNote: true, isMany: false,
        note: mode === 'staged' ? 'All 3 files staged, ready to commit' : '3 files changed, nothing staged',
        filesLabel: mode === 'staged' ? 'Staged' : 'Not staged', files: mark(w.files), stat: sum(w.files), diffHeader: w.header, diff: w.diff };
    } else if (mode === 'many') {
      var seen = {}, union = [];
      sel.forEach(function (i) { FILES[C[i].set].files.forEach(function (f) { if (!seen[f.path]) { seen[f.path] = 1; union.push(f); } }); });
      var top = FILES[C[sel[0]].set];
      var what = canBisect ? 'A linear range: bisect or squash' : canSquash ? 'Consecutive on one branch: can be squashed' : 'Not consecutive on one branch: cherry-pick only';
      ins = { kicker: 'Selection', title: n + ' commits', isCommit: false, isNote: true, isMany: true, note: what,
        list: sel.map(function (i) { return { sha: C[i].commit.sha.slice(0, 7), subject: C[i].commit.subject }; }),
        filesLabel: 'Changed files', files: mark(union), stat: sum(union), diffHeader: top.header, diff: top.diff };
    } else {
      var idx = mode === 'one' ? sel[0] : 0;
      var s = C[idx], fs = FILES[s.set];
      ins = { kicker: mode === 'one' ? 'Commit' : 'Head of main', title: s.commit.subject, isCommit: true, isNote: false, isMany: false,
        author: s.commit.author, initial: s.commit.author.charAt(0), laneVar: 'var(--lane-' + s.commit.authorLane + ')',
        when: s.when, sha: s.commit.sha, parent: s.parent, body: s.body,
        filesLabel: 'Changed files', files: mark(fs.files), stat: sum(fs.files), diffHeader: fs.header, diff: fs.diff };
    }

    var mkRepos = function (list) {
      return list.map(function (r) { return Object.assign({ ahead: 0, behind: 0, changes: 0 }, r, { selected: r.name === (self.state.repo || 'atlas-web'), pick: function () { self.setState({ repo: r.name }); } }); });
    };
    var pinned = mkRepos([
      { name: 'atlas-web', owner: 'studio', branch: 'main', lane: 1, ahead: 2, changes: 3 },
      { name: 'rune-playground', owner: 'personal', branch: 'spring-tuning', lane: 2 }
    ]);
    var repos = mkRepos([
      { name: 'payments-api', owner: 'studio', branch: 'release/2.3', lane: 3, behind: 4 },
      { name: 'brand-tokens', owner: 'studio', branch: 'main', lane: 4, changes: 12 },
      { name: 'dotfiles', owner: 'personal', branch: 'main', lane: 5 },
      { name: 'field-notes', owner: 'personal', branch: 'drafts', lane: 6 },
      { name: 'infra-terraform', owner: 'studio', branch: 'main', lane: 1, behind: 1 }
    ]);
    var all = pinned.concat(repos);
    var repo = all.filter(function (r) { return r.selected; })[0] || all[0];

    return {
      theme: theme,
      s12: 12,
      s14: 14,
      pinned: pinned,
      repos: repos,
      repo: repo,
      commits: commits,
      inspectorOptions: [{ value: 'message', label: 'Message' }, { value: 'files', label: 'Files' }],
      ctx: {
        idle: mode === 'idle', changes: mode === 'changes', staged: mode === 'staged', one: mode === 'one', many: mode === 'many',
        bisect: mode === 'many' && canBisect,
        squashPrimary: mode === 'many' && canSquash && !canBisect,
        squashSecondary: mode === 'many' && canSquash && canBisect,
        ucPressed: st.changes ? 'true' : 'false',
        ucFill: st.staged ? 'var(--accent)' : 'none',
        ucDash: st.staged ? '0' : '2.5 2',
        ucNote: st.staged ? 'Staged' : ''
      },
      ins: ins,
      pickChanges: function () { self.setState(self.state.changes ? { changes: false } : { changes: true, sel: [], anchor: null }); },
      stageAll: function () { self.setState({ staged: true }); },
      unstageAll: function () { self.setState({ staged: false }); }
    };
  }
}

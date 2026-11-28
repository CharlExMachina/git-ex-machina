//! What a conflict sits in, as a person names it in a list: `fn refresh`, `class Cache`,
//! `impl Sidebar`. A best-effort scan upward from the conflict for the nearest declaration that
//! encloses it, by indentation, in the languages people write most (Rust, Python, JavaScript
//! and TypeScript, Go, Ruby, Java, Kotlin, Swift, C#, PHP). No parser: a wrong guess costs a
//! label, never a resolution.

/// The keywords that open a declaration, and whether the name after them is all the label needs
/// (`fn name`), or the rest of the line up to its brace (`impl Trait for Type`).
const KEYWORDS: &[(&str, bool)] = &[
    ("fn", true),
    ("def", true),
    ("function", true),
    ("func", true),
    ("fun", true),
    ("class", true),
    ("struct", true),
    ("enum", true),
    ("trait", true),
    ("interface", true),
    ("protocol", true),
    ("extension", true),
    ("object", true),
    ("module", true),
    ("mod", true),
    ("namespace", true),
    ("type", true),
    ("impl", false),
];

/// Words that may come before a declaration's keyword: visibility and qualifiers.
const QUALIFIERS: &[&str] = &[
    "pub",
    "pub(crate)",
    "pub(super)",
    "export",
    "default",
    "async",
    "unsafe",
    "const",
    "static",
    "public",
    "private",
    "protected",
    "internal",
    "abstract",
    "final",
    "sealed",
    "open",
    "override",
    "data",
    "inline",
    "extern",
    "\"C\"",
];

/// How deep a line is indented, a tab counting as four spaces.
fn indent(line: &str) -> usize {
    line.chars().take_while(|c| c.is_whitespace()).map(|c| if c == '\t' { 4 } else { 1 }).sum()
}

/// The declaration `line` opens, named as a list shows it, if it opens one.
pub fn declaration(line: &str) -> Option<String> {
    // Generics out first, so `impl<T: Clone>` reads as `impl`.
    let line = strip_generics(line);
    let mut words = line.split_whitespace().peekable();
    while words.peek().is_some_and(|w| QUALIFIERS.contains(w)) {
        words.next();
    }
    let keyword = words.next()?.trim_end_matches('*');
    let &(keyword, name_only) = KEYWORDS.iter().find(|(k, _)| *k == keyword)?;
    let rest: Vec<&str> = words.collect();
    let rest = rest.join(" ");
    if !name_only {
        let head = rest.split(['{', ';']).next().unwrap_or_default().trim();
        return (!head.is_empty()).then(|| format!("{keyword} {head}"));
    }
    // Go's methods name their receiver first: `func (c *Cache) Refresh(`.
    let rest = if keyword == "func" && rest.starts_with('(') {
        rest.split_once(')').map_or("", |(_, r)| r).trim_start()
    } else {
        rest.as_str()
    };
    // A generator (`function* name`) and Kotlin's extension receivers (`fun Cache.load`).
    let rest = rest.trim_start_matches('*').trim_start();
    let name: String = rest.chars().take_while(|c| c.is_alphanumeric() || matches!(c, '_' | '$' | '.')).collect();
    let name = name.rsplit('.').next().unwrap_or_default();
    if name.is_empty() || name.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return None;
    }
    Some(format!("{keyword} {name}"))
}

/// `Foo<T: Clone> for Bar<T>` as `Foo for Bar`.
fn strip_generics(s: &str) -> String {
    let mut depth = 0usize;
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '<' => depth += 1,
            '>' => depth = depth.saturating_sub(1),
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The declaration enclosing a conflict whose lines are `inside`, scanning `above` (the file's
/// lines before it, nearest last). It's the nearest declaration indented less than the
/// conflict; for a conflict at the left margin, the nearest at all.
pub fn enclosing<'a>(above: impl DoubleEndedIterator<Item = &'a str>, inside: &[&str]) -> Option<String> {
    let depth = inside.iter().filter(|l| !l.trim().is_empty()).map(|l| indent(l)).min().unwrap_or(0);
    for line in above.rev() {
        if line.trim().is_empty() {
            continue;
        }
        let d = indent(line);
        if (d < depth || depth == 0)
            && let Some(name) = declaration(line)
        {
            return Some(name);
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declarations_by_language() {
        let cases = [
            ("pub fn refresh(&mut self) {", Some("fn refresh")),
            ("    pub(crate) async fn load_all<T>(ids: &[T]) -> Vec<T> {", Some("fn load_all")),
            ("impl<T: Clone> Cache<T> for Avatars {", Some("impl Cache for Avatars")),
            ("impl Sidebar {", Some("impl Sidebar")),
            ("    def evict(self, key):", Some("def evict")),
            ("export default async function render() {", Some("function render")),
            ("function* walk(tree) {", Some("function walk")),
            ("func (c *Cache) Refresh(ctx context.Context) error {", Some("func Refresh")),
            ("public final class AvatarCache extends Cache {", Some("class AvatarCache")),
            ("class Cache < Base", Some("class Cache")),
            ("fun Cache.loadAll(): List<Avatar> {", Some("fun loadAll")),
            ("pub mod springs {", Some("mod springs")),
            ("struct Row {", Some("struct Row")),
            ("let fn_count = 3;", None),
            ("// fn commented out", None),
            ("type", None),
            ("    return function_call(x);", None),
        ];
        for (line, want) in cases {
            assert_eq!(declaration(line).as_deref(), want, "{line:?}");
        }
    }

    #[test]
    fn the_nearest_declaration_that_encloses_the_conflict() {
        let file = [
            "impl Cache {",
            "    fn evict(&mut self) {",
            "        self.lru.pop();",
            "    }",
            "",
            "    fn refresh(&mut self) {",
            "        let stale = self.stale();",
        ];
        // Inside refresh's body.
        assert_eq!(enclosing(file.iter().copied(), &["        self.load(stale);"]).as_deref(), Some("fn refresh"));
        // A conflict that is a whole method sits in the impl, not in the method above it.
        assert_eq!(enclosing(file[..5].iter().copied(), &["    fn load(&mut self) {"]).as_deref(), Some("impl Cache"));
        // At the margin, the nearest declaration above is the best guess.
        let top = ["mod springs {", "// motion 9"];
        assert_eq!(enclosing(top.iter().copied(), &["pub const SMOOTH: f32 = 0.38;"]).as_deref(), Some("mod springs"));
        // Nothing declares anything: no label.
        let json = ["{", "  \"accent\": \"#e2ad62\","];
        assert_eq!(enclosing(json.iter().copied(), &["  \"spring\": \"smooth\","]), None);
        assert_eq!(enclosing(std::iter::empty(), &["x"]), None);
    }
}

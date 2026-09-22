/// Python `fnmatch.fnmatchcase` 互換のグロブ判定 (SPEC §4.10)。
///
/// `glob` / `globset` クレートは `*` が `/` を跨がないため使えない。
/// ここでは `*` = 任意の文字列 (`/` を含む)、`?` = 任意の 1 文字、`[abc]` / `[!abc]` = 文字クラス、
/// という Python 仕様をそのまま再現する。
pub(crate) fn fnmatch(name: &str, pattern: &str) -> bool {
    let n: Vec<char> = name.chars().collect();
    let p: Vec<char> = pattern.chars().collect();
    is_match(&n, &p)
}

/// パターンが glob 文字を含むか (含めば複数実行モードに入る)。
pub(crate) fn has_magic(pattern: &str) -> bool {
    pattern.contains(['*', '?', '['])
}

fn is_match(name: &[char], pat: &[char]) -> bool {
    // 末尾まで来たら、残りのパターンが空か `*` だけなら一致。
    let Some(&first) = pat.first() else {
        return name.is_empty();
    };

    match first {
        '*' => {
            // `*` は 0 文字以上に一致する。`/` も跨ぐ (Python 仕様)。
            for split in 0..=name.len() {
                if is_match(&name[split..], &pat[1..]) {
                    return true;
                }
            }
            false
        }
        '?' => !name.is_empty() && is_match(&name[1..], &pat[1..]),
        '[' => match parse_class(pat) {
            // 閉じ括弧が無い `[` はリテラル扱い (Python 仕様)。
            None => !name.is_empty() && name[0] == '[' && is_match(&name[1..], &pat[1..]),
            Some((set, negated, consumed)) => {
                if name.is_empty() {
                    return false;
                }
                let hit = set.contains(&name[0]);
                (hit != negated) && is_match(&name[1..], &pat[consumed..])
            }
        },
        c => !name.is_empty() && name[0] == c && is_match(&name[1..], &pat[1..]),
    }
}

/// `[...]` を解析して (文字集合, 否定か, 消費した文字数) を返す。閉じられていなければ None。
fn parse_class(pat: &[char]) -> Option<(Vec<char>, bool, usize)> {
    let mut i = 1;
    let negated = matches!(pat.get(i), Some('!'));
    if negated {
        i += 1;
    }
    // 先頭の `]` はリテラル扱い (Python 仕様)。
    if matches!(pat.get(i), Some(']')) {
        i += 1;
    }
    while i < pat.len() && pat[i] != ']' {
        i += 1;
    }
    if i >= pat.len() {
        return None;
    }

    let body_start = 1 + usize::from(negated);
    let mut set = Vec::new();
    let body = &pat[body_start..i];
    let mut j = 0;
    while j < body.len() {
        // `a-z` のような範囲指定を展開する。
        if j + 2 < body.len() && body[j + 1] == '-' {
            for c in body[j]..=body[j + 2] {
                set.push(c);
            }
            j += 3;
        } else {
            set.push(body[j]);
            j += 1;
        }
    }
    Some((set, negated, i + 1))
}

#[cfg(test)]
// テスト名は日本語で書く方針のため、snake_case 検査から除外する
#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn glob_アスタリスクはスラッシュを跨ぐ() {
        // Python fnmatch の仕様。glob クレートとはここが違う
        assert!(fnmatch("Battle/Repro/X", "Battle/*"));
        assert!(fnmatch("Battle/X", "Battle/*"));
    }

    #[test]
    fn glob_完全一致() {
        assert!(fnmatch("Foo/Bar", "Foo/Bar"));
        assert!(!fnmatch("Foo/Bar", "Foo/Baz"));
    }

    #[test]
    fn glob_疑問符は1文字() {
        assert!(fnmatch("Foo1", "Foo?"));
        assert!(!fnmatch("Foo12", "Foo?"));
        assert!(!fnmatch("Foo", "Foo?"));
    }

    #[test]
    fn glob_文字クラス() {
        assert!(fnmatch("Foo1", "Foo[123]"));
        assert!(!fnmatch("Foo4", "Foo[123]"));
    }

    #[test]
    fn glob_文字クラスの否定() {
        assert!(fnmatch("Foo4", "Foo[!123]"));
        assert!(!fnmatch("Foo1", "Foo[!123]"));
    }

    #[test]
    fn glob_文字クラスの範囲() {
        assert!(fnmatch("Foob", "Foo[a-c]"));
        assert!(!fnmatch("Food", "Foo[a-c]"));
    }

    #[test]
    fn glob_閉じない括弧はリテラル() {
        assert!(fnmatch("Foo[", "Foo["));
    }

    #[test]
    fn glob_先頭アスタリスクと複数アスタリスク() {
        assert!(fnmatch("a/b/c", "*"));
        assert!(fnmatch("a/b/c", "*/*/*"));
        assert!(fnmatch("a/b/c", "a/*/c"));
        assert!(fnmatch("abc", "*b*"));
    }

    #[test]
    fn glob_空文字() {
        assert!(fnmatch("", "*"));
        assert!(fnmatch("", ""));
        assert!(!fnmatch("", "?"));
    }

    #[test]
    fn glob_大文字小文字は区別する() {
        // fnmatchcase 相当なので case-sensitive
        assert!(!fnmatch("foo", "FOO"));
    }

    #[test]
    fn has_magic_判定() {
        assert!(has_magic("Battle/*"));
        assert!(has_magic("Foo?"));
        assert!(has_magic("Foo[1]"));
        assert!(!has_magic("Foo/Bar"));
    }
}

//! `.worktreeinclude` — which gitignored files a fresh checkout carries
//! (Claude Code's file, <https://code.claude.com/docs/en/worktrees>). Git
//! enumerates the ignored files, so this only matches `.gitignore` syntax
//! against them: `#` comments, `!` negation (last match wins), anchoring by
//! `/`, trailing-`/` directories, `*`, `**`, `?`, `[...]`, `\` escapes. Not
//! byte-for-byte git parity; a pattern this cannot read matches nothing.

/// A parsed `.worktreeinclude`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patterns(Vec<Pattern>);

#[derive(Debug, Clone, PartialEq, Eq)]
struct Pattern {
    negated: bool,
    /// Matched from the root; any `/` but a trailing one anchors.
    anchored: bool,
    /// `pattern/`: only a candidate under the directory matches.
    dir_only: bool,
    /// One glob per path component; `**` spans any number, including none.
    pieces: Vec<Piece>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Piece {
    Any,
    Glob(Vec<Token>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Token {
    Literal(char),
    Star,
    One,
    /// `[...]`; `negated` for `[!...]` and `[^...]`.
    Class {
        negated: bool,
        ranges: Vec<(char, char)>,
    },
}

impl Patterns {
    /// Lines this cannot make sense of are dropped rather than guessed at.
    pub fn parse(text: &str) -> Self {
        Self(text.lines().filter_map(Pattern::parse).collect())
    }

    /// Whether a repository-relative, `/`-separated file path is included; the
    /// last matching pattern decides.
    pub fn matches(&self, repo_relative: &str) -> bool {
        let path = repo_relative.trim_start_matches("./").trim_end_matches('/');
        if path.is_empty() {
            return false;
        }
        let components: Vec<&str> = path.split('/').filter(|c| !c.is_empty()).collect();
        let mut included = false;
        for p in &self.0 {
            if p.matches(&components) {
                included = !p.negated;
            }
        }
        included
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Pattern {
    fn parse(line: &str) -> Option<Self> {
        let mut text = trim_trailing_spaces(line);
        if text.is_empty() || text.starts_with('#') {
            return None;
        }
        let mut negated = false;
        if let Some(rest) = text.strip_prefix('!') {
            negated = true;
            text = rest;
        }
        let mut dir_only = false;
        if let Some(rest) = text.strip_suffix('/') {
            dir_only = true;
            text = rest;
        }
        let mut anchored = false;
        if let Some(rest) = text.strip_prefix('/') {
            anchored = true;
            text = rest;
        }
        if text.contains('/') {
            anchored = true;
        }
        if text.is_empty() {
            return None;
        }
        let pieces: Vec<Piece> = text.split('/').map(Piece::parse).collect();
        Some(Self {
            negated,
            anchored,
            dir_only,
            pieces,
        })
    }

    /// The file and every directory above it are tried, since a matched
    /// directory takes everything under it; `dir_only` tries the directories alone.
    fn matches(&self, components: &[&str]) -> bool {
        let deepest = if self.dir_only {
            components.len().saturating_sub(1)
        } else {
            components.len()
        };
        (1..=deepest).any(|len| self.matches_exactly(&components[..len]))
    }

    fn matches_exactly(&self, components: &[&str]) -> bool {
        if self.anchored {
            return match_pieces(&self.pieces, components);
        }
        // Unanchored: `name` means `**/name`.
        (0..=components.len()).any(|skip| match_pieces(&self.pieces, &components[skip..]))
    }
}

/// Strips unescaped trailing spaces; a `\ ` at the end is a space in the name.
fn trim_trailing_spaces(line: &str) -> &str {
    let bytes = line.as_bytes();
    let mut end = line.len();
    while end > 0 && bytes[end - 1] == b' ' {
        if end >= 2 && bytes[end - 2] == b'\\' {
            break;
        }
        end -= 1;
    }
    &line[..end]
}

/// Matches pieces against components, with `**` free to absorb any number.
fn match_pieces(pieces: &[Piece], components: &[&str]) -> bool {
    match pieces.split_first() {
        None => components.is_empty(),
        Some((Piece::Any, rest)) => {
            (0..=components.len()).any(|skip| match_pieces(rest, &components[skip..]))
        }
        Some((Piece::Glob(tokens), rest)) => match components.split_first() {
            Some((first, more)) => match_tokens(tokens, first) && match_pieces(rest, more),
            None => false,
        },
    }
}

impl Piece {
    fn parse(text: &str) -> Self {
        if text == "**" {
            return Piece::Any;
        }
        let mut tokens = Vec::new();
        let mut chars = text.chars().peekable();
        while let Some(c) = chars.next() {
            match c {
                '\\' => {
                    tokens.push(Token::Literal(chars.next().unwrap_or('\\')));
                }
                // `a**b` inside one component is an ordinary `*`.
                '*' => {
                    while chars.peek() == Some(&'*') {
                        chars.next();
                    }
                    tokens.push(Token::Star);
                }
                '?' => tokens.push(Token::One),
                '[' => {
                    let mut negated = false;
                    if matches!(chars.peek(), Some('!') | Some('^')) {
                        negated = true;
                        chars.next();
                    }
                    let mut ranges = Vec::new();
                    let mut closed = false;
                    let mut first = true;
                    while let Some(c) = chars.next() {
                        // A `]` first in the set is a literal member.
                        if c == ']' && !first {
                            closed = true;
                            break;
                        }
                        first = false;
                        let lo = if c == '\\' {
                            chars.next().unwrap_or('\\')
                        } else {
                            c
                        };
                        if chars.peek() == Some(&'-') {
                            let mut look = chars.clone();
                            look.next();
                            match look.next() {
                                Some(hi) if hi != ']' => {
                                    chars.next();
                                    chars.next();
                                    ranges.push((lo, hi));
                                    continue;
                                }
                                _ => {}
                            }
                        }
                        ranges.push((lo, lo));
                    }
                    if closed {
                        tokens.push(Token::Class { negated, ranges });
                    } else {
                        // An unclosed bracket is a literal `[`, as in git.
                        tokens.push(Token::Literal('['));
                        for (lo, _) in ranges {
                            tokens.push(Token::Literal(lo));
                        }
                    }
                }
                other => tokens.push(Token::Literal(other)),
            }
        }
        Piece::Glob(tokens)
    }
}

/// Glob over one path component; `*` never sees a `/` because there is none.
fn match_tokens(tokens: &[Token], text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    match_at(tokens, &chars)
}

fn match_at(tokens: &[Token], chars: &[char]) -> bool {
    match tokens.split_first() {
        None => chars.is_empty(),
        Some((Token::Star, rest)) => (0..=chars.len()).any(|n| match_at(rest, &chars[n..])),
        Some((Token::One, rest)) => !chars.is_empty() && match_at(rest, &chars[1..]),
        Some((Token::Literal(c), rest)) => chars.first() == Some(c) && match_at(rest, &chars[1..]),
        Some((Token::Class { negated, ranges }, rest)) => match chars.first() {
            Some(c) => {
                let inside = ranges.iter().any(|(lo, hi)| lo <= c && c <= hi);
                inside != *negated && match_at(rest, &chars[1..])
            }
            None => false,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn included(file: &str, path: &str) -> bool {
        Patterns::parse(file).matches(path)
    }

    #[test]
    fn a_bare_name_matches_at_any_depth_and_only_that_name() {
        assert!(included(".env\n", ".env"));
        assert!(included(".env\n", "apps/web/.env"));
        assert!(!included(".env\n", ".env.local"), "a name is not a prefix");
        assert!(!included(".env\n", "env"));
    }

    #[test]
    fn a_leading_slash_anchors_to_the_root() {
        assert!(included("/config/secrets.json\n", "config/secrets.json"));
        assert!(
            !included("/config/secrets.json\n", "apps/config/secrets.json"),
            "anchored means the root, not any depth"
        );
    }

    #[test]
    fn an_inner_slash_anchors_and_a_star_does_not_cross_a_slash() {
        assert!(included("config/*.local\n", "config/a.local"));
        assert!(!included("config/*.local\n", "config/a/b.local"));
        assert!(
            !included("config/*.local\n", "apps/config/a.local"),
            "an inner slash anchors too"
        );
    }

    #[test]
    fn a_double_star_crosses_directories() {
        for path in [
            "vendor/config.json",
            "vendor/a/config.json",
            "vendor/a/b/c/config.json",
        ] {
            assert!(included("vendor/**/config.json\n", path), "{path}");
        }
        assert!(!included("vendor/**/config.json\n", "other/config.json"));
        assert!(!included("vendor/**/config.json\n", "vendor/config.yaml"));

        for path in ["config.json", "a/config.json", "a/b/c/config.json"] {
            assert!(included("**/config.json\n", path), "{path}");
        }
        assert!(!included("**/config.json\n", "a/config.jsonx"));
    }

    #[test]
    fn negation_after_a_match_excludes_and_the_last_match_wins() {
        let file = "*.env\n!keep.env\n";
        assert!(included(file, "one.env"));
        assert!(!included(file, "keep.env"), "the later `!` wins");
        assert!(!included(file, "sub/keep.env"));
        let file = "*.env\n!keep.env\nkeep.env\n";
        assert!(included(file, "keep.env"));
        // A negation with nothing before it includes nothing.
        assert!(!included("!keep.env\n", "keep.env"));
        assert!(!included("!keep.env\n", "other.env"));
    }

    #[test]
    fn a_trailing_slash_names_a_directory_and_takes_what_is_under_it() {
        assert!(included("build/\n", "build/out/a.o"));
        assert!(included("build/\n", "build/a.o"));
        assert!(
            !included("build/\n", "build"),
            "a file called `build` is not a directory"
        );
        assert!(!included("build/\n", "src/a.o"));
        // Without the slash a matched directory still takes everything under it.
        assert!(included("build\n", "build/out/a.o"));
        assert!(included("build\n", "build"));
    }

    #[test]
    fn comments_and_blank_lines_match_nothing() {
        assert!(!included("# .env\n", ".env"));
        assert!(!included("\n\n   \n", ".env"));
        assert!(!included("#\n", "#"));
        assert!(Patterns::parse("# only a comment\n\n").is_empty());
    }

    #[test]
    fn a_backslash_escapes_the_next_character() {
        assert!(included("a\\#b\n", "a#b"));
        assert!(!included("a\\#b\n", "ab"));
        assert!(
            included("\\#literal\n", "#literal"),
            "an escaped `#` is a name, not a comment"
        );
        assert!(
            included("\\!bang\n", "!bang"),
            "an escaped `!` is a name, not a negation"
        );
        assert!(!included("\\!bang\n", "bang"));
        assert!(included("star\\*\n", "star*"));
        assert!(!included("star\\*\n", "starx"));
    }

    #[test]
    fn a_question_mark_is_exactly_one_character() {
        assert!(included("?.env\n", "a.env"));
        assert!(!included("?.env\n", "ab.env"));
        assert!(!included("?.env\n", ".env"));
    }

    #[test]
    fn a_class_is_one_character_from_the_set() {
        assert!(included("[ab].env\n", "a.env"));
        assert!(included("[a-c].env\n", "c.env"));
        assert!(!included("[a-c].env\n", "d.env"));
        assert!(included("[!a].env\n", "b.env"));
        assert!(!included("[!a].env\n", "a.env"));
        assert!(!included("[ab].env\n", "ab.env"));
    }

    #[test]
    fn trailing_spaces_are_ignored_unless_escaped() {
        assert!(included(".env   \n", ".env"));
        assert!(included("name\\ \n", "name "));
        assert!(!included("name\\ \n", "name"));
    }

    #[test]
    fn a_leading_dot_slash_on_the_candidate_is_tolerated() {
        assert!(included(".env\n", "./.env"));
        assert!(!included(".env\n", ""));
    }
}

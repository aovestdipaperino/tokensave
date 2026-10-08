//! `ActiveSupport`'s default English inflections, for the lowercase
//! `[a-z0-9_]` resource names the route extractor accepts. The rules are
//! checked in `ActiveSupport`'s order (irregulars first, then the newest
//! rule first) and the first match wins. Applications that customize the
//! inflector are detected separately and leave their routes unlinked.

const UNCOUNTABLE: [&str; 10] = [
    "equipment",
    "information",
    "rice",
    "money",
    "species",
    "series",
    "fish",
    "sheep",
    "jeans",
    "police",
];

/// `(singular, plural)`, newest first as `ActiveSupport` checks them.
const IRREGULAR: [(&str, &str); 6] = [
    ("zombie", "zombies"),
    ("move", "moves"),
    ("sex", "sexes"),
    ("child", "children"),
    ("man", "men"),
    ("person", "people"),
];

fn replace_suffix(word: &str, suffix: &str, replacement: &str) -> String {
    format!("{}{replacement}", &word[..word.len() - suffix.len()])
}

/// The character before a suffix of `len` bytes, if any.
fn before(word: &str, len: usize) -> Option<u8> {
    word.len()
        .checked_sub(len + 1)
        .map(|index| word.as_bytes()[index])
}

fn consonant_or_qu(word: &str, len: usize) -> bool {
    before(word, len).is_some_and(|c| !b"aeiouy".contains(&c))
        || word[..word.len() - len].ends_with("qu")
}

pub(crate) fn pluralize(word: &str) -> String {
    if word.is_empty() || UNCOUNTABLE.contains(&word) {
        return word.to_string();
    }
    for (singular, plural) in IRREGULAR {
        if word.ends_with(plural) {
            return word.to_string();
        }
        if word.ends_with(singular) {
            return replace_suffix(word, singular, plural);
        }
    }
    let ends = |suffix: &str| word.ends_with(suffix);
    if ends("quiz") {
        return format!("{word}zes");
    }
    if word == "oxen" || word == "mice" || word == "lice" {
        return word.to_string();
    }
    if word == "ox" {
        return "oxen".into();
    }
    if word == "mouse" || word == "louse" {
        return format!("{}ice", &word[..1]);
    }
    for stem in ["matr", "vert", "ind"] {
        for tail in ["ix", "ex"] {
            if ends(&format!("{stem}{tail}")) {
                return replace_suffix(word, tail, "ices");
            }
        }
    }
    if ["x", "ch", "ss", "sh"].iter().any(|s| ends(s)) {
        return format!("{word}es");
    }
    if ends("y") && consonant_or_qu(word, 1) {
        return replace_suffix(word, "y", "ies");
    }
    if ends("hive") {
        return format!("{word}s");
    }
    if ends("fe") && before(word, 2).is_some_and(|c| c != b'f') {
        return replace_suffix(word, "fe", "ves");
    }
    if ends("f") && before(word, 1).is_some_and(|c| c == b'l' || c == b'r') {
        return replace_suffix(word, "f", "ves");
    }
    if ends("sis") {
        return replace_suffix(word, "sis", "ses");
    }
    if (ends("ta") || ends("ia")) && word.len() >= 2 {
        return word.to_string();
    }
    if ends("tum") || ends("ium") {
        return replace_suffix(word, "um", "a");
    }
    if ends("buffalo") || ends("tomato") {
        return format!("{word}es");
    }
    if ends("bus") || ends("alias") || ends("status") {
        return format!("{word}es");
    }
    if ends("octopi") || ends("viri") {
        return word.to_string();
    }
    if ends("octopus") || ends("virus") {
        return replace_suffix(word, "us", "i");
    }
    if word == "axis" || word == "testis" {
        return replace_suffix(word, "is", "es");
    }
    if ends("s") {
        return word.to_string();
    }
    format!("{word}s")
}

#[cfg_attr(not(feature = "lang-ruby"), allow(dead_code))]
pub(crate) fn singularize(word: &str) -> String {
    if word.is_empty() || UNCOUNTABLE.contains(&word) {
        return word.to_string();
    }
    for (singular, plural) in IRREGULAR {
        if word.ends_with(plural) {
            return replace_suffix(word, plural, singular);
        }
        if word.ends_with(singular) {
            return word.to_string();
        }
    }
    let ends = |suffix: &str| word.ends_with(suffix);
    if ends("databases") {
        return replace_suffix(word, "s", "");
    }
    if ends("quizzes") {
        return replace_suffix(word, "zes", "");
    }
    if ends("matrices") {
        return replace_suffix(word, "ices", "ix");
    }
    if ends("vertices") || ends("indices") {
        return replace_suffix(word, "ices", "ex");
    }
    if let Some(rest) = word.strip_prefix("oxen") {
        return format!("ox{rest}");
    }
    if ends("aliases") || ends("statuses") {
        return replace_suffix(word, "es", "");
    }
    if ends("alias") || ends("status") {
        return word.to_string();
    }
    for stem in ["octop", "vir"] {
        for tail in ["us", "i"] {
            if ends(&format!("{stem}{tail}")) {
                return replace_suffix(word, tail, "us");
            }
        }
    }
    if word == "axis" || word == "axes" {
        return "axis".into();
    }
    for stem in ["cris", "test"] {
        for tail in ["is", "es"] {
            if ends(&format!("{stem}{tail}")) {
                return replace_suffix(word, tail, "is");
            }
        }
    }
    if ends("shoes") {
        return replace_suffix(word, "s", "");
    }
    if ends("oes") {
        return replace_suffix(word, "es", "");
    }
    if ends("buses") {
        return replace_suffix(word, "es", "");
    }
    if ends("bus") {
        return word.to_string();
    }
    if word == "mice" || word == "lice" {
        return format!("{}ouse", &word[..1]);
    }
    if ["xes", "ches", "sses", "shes"].iter().any(|s| ends(s)) {
        return replace_suffix(word, "es", "");
    }
    if ends("movies") {
        return replace_suffix(word, "s", "");
    }
    if ends("series") {
        return word.to_string();
    }
    if ends("ies") && consonant_or_qu(word, 3) {
        return replace_suffix(word, "ies", "y");
    }
    if ends("ves") && before(word, 3).is_some_and(|c| c == b'l' || c == b'r') {
        return replace_suffix(word, "ves", "f");
    }
    if ends("tives") || ends("hives") {
        return replace_suffix(word, "s", "");
    }
    if ends("ves") && before(word, 3).is_some_and(|c| c != b'f') {
        return replace_suffix(word, "ves", "fe");
    }
    for stem in [
        "analy", "ba", "diagno", "parenthe", "progno", "synop", "the",
    ] {
        for tail in ["sis", "ses"] {
            if ends(&format!("{stem}{tail}")) {
                return replace_suffix(word, tail, "sis");
            }
        }
    }
    if ends("ta") || ends("ia") {
        return replace_suffix(word, "a", "um");
    }
    if ends("news") || ends("ss") {
        return word.to_string();
    }
    if ends("s") {
        return replace_suffix(word, "s", "");
    }
    word.to_string()
}

/// `ActiveSupport::Inflector.underscore` without acronyms: `Foo::BarBaz`
/// becomes `foo/bar_baz` and `HTMLKit` becomes `html_kit`.
pub(crate) fn underscore(constant: &str) -> String {
    let chars: Vec<char> = constant.replace("::", "/").chars().collect();
    let mut out = String::new();
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && c.is_ascii_uppercase() {
            let prev = chars[i - 1];
            let next_lower = chars.get(i + 1).is_some_and(char::is_ascii_lowercase);
            if (prev.is_ascii_uppercase() && next_lower)
                || prev.is_ascii_lowercase()
                || prev.is_ascii_digit()
            {
                out.push('_');
            }
        }
        out.push(if c == '-' {
            '_'
        } else {
            c.to_ascii_lowercase()
        });
    }
    out
}

/// `"admin/user_notes".camelize`: `Admin::UserNotes`.
pub(crate) fn camelize(path: &str) -> String {
    path.split('/')
        .map(|part| {
            part.split('_')
                .filter(|s| !s.is_empty())
                .map(|word| {
                    let mut chars = word.chars();
                    chars.next().map_or_else(String::new, |first| {
                        format!("{}{}", first.to_ascii_uppercase(), chars.as_str())
                    })
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("::")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Expected values printed by `ActiveSupport` 8.1.3.1 for each word.
    #[test]
    fn matches_activesupport_default_inflections() {
        for (word, plural, singular) in include!("rails_inflector_cases.in") {
            assert_eq!(pluralize(word), plural, "pluralize {word}");
            assert_eq!(singularize(word), singular, "singularize {word}");
        }
    }

    #[test]
    fn underscore_matches_activesupport() {
        for (constant, expected) in [
            ("Foo", "foo"),
            ("Foo::Bar", "foo/bar"),
            ("MyEngine", "my_engine"),
            ("HTMLKit", "html_kit"),
            ("Api2Things", "api2_things"),
            ("ABC", "abc"),
            ("Foo::HTTPServer", "foo/http_server"),
        ] {
            assert_eq!(underscore(constant), expected, "{constant}");
        }
    }
}

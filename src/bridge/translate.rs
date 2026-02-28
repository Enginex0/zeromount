use crate::core::config::UnameMode;

pub(super) fn bool_to_int(v: bool) -> u8 {
    if v { 1 } else { 0 }
}

pub(super) fn int_to_bool(v: u8) -> bool {
    v >= 1
}

fn string_to_quoted(s: &str) -> String {
    format!("'{s}'")
}

fn quoted_to_string(s: &str) -> String {
    s.trim_matches('\'').to_string()
}

// susfs4ksu: single int encodes uname mode
pub(super) fn uname_mode_to_susfs4ksu(mode: &UnameMode) -> u8 {
    match mode {
        UnameMode::Disabled => 0,
        UnameMode::Static => 1,
        UnameMode::Dynamic => 2,
    }
}

pub(super) fn uname_mode_from_susfs4ksu(v: u8) -> UnameMode {
    match v {
        0 => UnameMode::Disabled,
        1 | 2 => UnameMode::Static,
        _ => UnameMode::Disabled,
    }
}

// BRENE: 3 mutually exclusive booleans encode uname mode + release context
pub(super) fn uname_mode_to_brene_triple(mode: &UnameMode, release: &str) -> (u8, u8, u8) {
    let has_custom_release = !release.is_empty() && release != "default";
    match mode {
        UnameMode::Disabled => (0, 0, 0),
        UnameMode::Static if !has_custom_release => (1, 0, 0),
        UnameMode::Static => (0, 1, 0),
        UnameMode::Dynamic => (0, 0, 1),
    }
}

pub(super) fn uname_mode_from_brene_triple(uname: u8, uname2: u8, custom: u8) -> UnameMode {
    if custom >= 1 {
        UnameMode::Dynamic
    } else if uname >= 1 || uname2 >= 1 {
        UnameMode::Static
    } else {
        UnameMode::Disabled
    }
}

// Normalize external string values on import
pub(super) fn normalize_string_value(s: &str) -> String {
    let stripped = quoted_to_string(s);
    if stripped == "default" {
        String::new()
    } else {
        stripped
    }
}

pub(super) fn string_to_external(s: &str) -> String {
    if s.is_empty() {
        string_to_quoted("default")
    } else {
        string_to_quoted(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bool_conversion_roundtrip() {
        assert_eq!(bool_to_int(true), 1);
        assert_eq!(bool_to_int(false), 0);
        assert!(int_to_bool(1));
        assert!(int_to_bool(2));
        assert!(!int_to_bool(0));
    }

    #[test]
    fn string_quoting_roundtrip() {
        assert_eq!(string_to_quoted("hello"), "'hello'");
        assert_eq!(quoted_to_string("'hello'"), "hello");
        assert_eq!(quoted_to_string("hello"), "hello");
    }

    #[test]
    fn susfs4ksu_uname_roundtrip() {
        assert_eq!(uname_mode_to_susfs4ksu(&UnameMode::Disabled), 0);
        assert_eq!(uname_mode_to_susfs4ksu(&UnameMode::Static), 1);
        assert_eq!(uname_mode_to_susfs4ksu(&UnameMode::Dynamic), 2);

        assert_eq!(uname_mode_from_susfs4ksu(0), UnameMode::Disabled);
        assert_eq!(uname_mode_from_susfs4ksu(1), UnameMode::Static);
        // susfs4ksu mode 2 also maps to static (spec: disabled=0, static=1, dynamic=2 -> but from_susfs4ksu collapses 1|2 to Static per spec 3a)
        assert_eq!(uname_mode_from_susfs4ksu(2), UnameMode::Static);
    }

    #[test]
    fn brene_triple_disabled() {
        assert_eq!(uname_mode_to_brene_triple(&UnameMode::Disabled, ""), (0, 0, 0));
        assert_eq!(uname_mode_to_brene_triple(&UnameMode::Disabled, "5.10"), (0, 0, 0));
    }

    #[test]
    fn brene_triple_static_default_release() {
        assert_eq!(uname_mode_to_brene_triple(&UnameMode::Static, ""), (1, 0, 0));
        assert_eq!(uname_mode_to_brene_triple(&UnameMode::Static, "default"), (1, 0, 0));
    }

    #[test]
    fn brene_triple_static_custom_release() {
        assert_eq!(uname_mode_to_brene_triple(&UnameMode::Static, "5.10.0-gki"), (0, 1, 0));
    }

    #[test]
    fn brene_triple_dynamic() {
        assert_eq!(uname_mode_to_brene_triple(&UnameMode::Dynamic, ""), (0, 0, 1));
        assert_eq!(uname_mode_to_brene_triple(&UnameMode::Dynamic, "5.10"), (0, 0, 1));
    }

    #[test]
    fn brene_triple_from_roundtrip() {
        assert_eq!(uname_mode_from_brene_triple(0, 0, 0), UnameMode::Disabled);
        assert_eq!(uname_mode_from_brene_triple(1, 0, 0), UnameMode::Static);
        assert_eq!(uname_mode_from_brene_triple(0, 1, 0), UnameMode::Static);
        assert_eq!(uname_mode_from_brene_triple(0, 0, 1), UnameMode::Dynamic);
    }

    #[test]
    fn string_normalization() {
        assert_eq!(normalize_string_value("'default'"), "");
        assert_eq!(normalize_string_value("'5.10.0'"), "5.10.0");
        assert_eq!(normalize_string_value("default"), "");
        assert_eq!(normalize_string_value("plain"), "plain");
    }

    #[test]
    fn string_to_external_encoding() {
        assert_eq!(string_to_external(""), "'default'");
        assert_eq!(string_to_external("5.10.0"), "'5.10.0'");
    }
}

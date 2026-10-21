//! Translations: RustPaint's Fluent files (`locales/`, converted from
//! HaikuPaint's catkeys by `tools/catkeys_to_ftl.py`), in the system's
//! language, falling back to English. RUSTPAINT_LANGUAGE overrides the
//! system's choice.

use fluent_bundle::{FluentArgs, FluentBundle, FluentResource};
use unic_langid::LanguageIdentifier;

const LOCALES: [(&str, &str); 5] = [
    ("en", include_str!("../../../locales/en.ftl")),
    ("de", include_str!("../../../locales/de.ftl")),
    ("es", include_str!("../../../locales/es.ftl")),
    ("fr", include_str!("../../../locales/fr.ftl")),
    ("sv", include_str!("../../../locales/sv.ftl")),
];

thread_local! {
    /// The bundles to look in, best language first, English last.
    static BUNDLES: Vec<FluentBundle<FluentResource>> = bundles(&requested());
}

/// The languages the user asked for. Tests always get English.
fn requested() -> Vec<LanguageIdentifier> {
    if cfg!(test) {
        return vec!["en".parse().expect("a language")];
    }
    let names: Vec<String> = match std::env::var("RUSTPAINT_LANGUAGE") {
        Ok(name) => vec![name],
        Err(_) => sys_locale::get_locales().collect(),
    };
    names.iter().filter_map(|name| name.parse().ok()).collect()
}

/// The languages RustPaint has of those requested, by their language
/// alone (Swiss German gets German), in order, then English.
fn bundles(requested: &[LanguageIdentifier]) -> Vec<FluentBundle<FluentResource>> {
    let mut codes: Vec<&str> = Vec::new();
    for language in requested {
        if let Some((code, _)) = LOCALES
            .iter()
            .find(|(c, _)| *c == language.language.as_str())
            && !codes.contains(code)
        {
            codes.push(code);
        }
    }
    if !codes.contains(&"en") {
        codes.push("en");
    }
    codes.into_iter().map(bundle).collect()
}

fn bundle(code: &str) -> FluentBundle<FluentResource> {
    let source = LOCALES
        .iter()
        .find(|(c, _)| *c == code)
        .map_or(LOCALES[0].1, |(_, source)| source);
    let resource = FluentResource::try_new(source.to_string()).expect("a valid .ftl file");
    let mut bundle = FluentBundle::new(vec![code.parse().expect("a language")]);
    // No bidi isolation marks around placeables; the UI is left to right.
    bundle.set_use_isolating(false);
    bundle.add_resource(resource).expect("unique messages");
    bundle
}

fn format(bundles: &[FluentBundle<FluentResource>], id: &str, args: &[(&str, &str)]) -> String {
    let mut fluent_args = FluentArgs::new();
    for &(name, value) in args {
        fluent_args.set(name, value.to_string());
    }
    for bundle in bundles {
        if let Some(pattern) = bundle.get_message(id).and_then(|m| m.value()) {
            let mut errors = Vec::new();
            return bundle
                .format_pattern(pattern, Some(&fluent_args), &mut errors)
                .into_owned();
        }
    }
    id.to_string()
}

/// The message, in the user's language.
pub fn tr(id: &str) -> String {
    tr_args(id, &[])
}

/// The message with its placeables filled in.
pub fn tr_args(id: &str, args: &[(&str, &str)]) -> String {
    BUNDLES.with(|bundles| format(bundles, id, args))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(source: &str) -> Vec<String> {
        source
            .lines()
            .filter(|line| !line.starts_with('#') && line.contains(" = "))
            .map(|line| line.split(" = ").next().unwrap().to_string())
            .collect()
    }

    #[test]
    fn every_language_has_every_message() {
        let english = ids(LOCALES[0].1);
        assert!(english.len() > 70);
        for (code, source) in &LOCALES[1..] {
            assert_eq!(ids(source), english, "{code}");
        }
    }

    #[test]
    fn messages_in_a_language_with_their_placeables() {
        let german = bundles(&["de-CH".parse().unwrap()]);
        assert_eq!(format(&german, "select-all", &[]), "Alles auswählen");
        assert_eq!(
            format(&german, "font-size-points", &[("size", "12")]),
            "12 Punkt"
        );
        // A language RustPaint doesn't have falls back to English.
        let japanese = bundles(&["ja".parse().unwrap()]);
        assert_eq!(format(&japanese, "select-all", &[]), "Select All");
        assert_eq!(
            format(&japanese, "save-changes", &[("name", "cat.mac")]),
            "Save changes to the document \"cat.mac\"?"
        );
        // An unknown message shows its id rather than nothing.
        assert_eq!(format(&japanese, "no-such-message", &[]), "no-such-message");
        assert_eq!(tr("ok"), "OK");
    }
}

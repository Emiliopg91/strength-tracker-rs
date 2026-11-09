use crate::utils::translations::{Languages, TRANSLATIONS, translate, translate_and_replace};

const ALL_LANGUAGES: [Languages; 2] = [Languages::Spanish, Languages::English];

#[test]
fn unknown_key_returns_the_key() {
    for lang in ALL_LANGUAGES {
        assert_eq!(
            translate("this_key_does_not_exist", lang),
            "this_key_does_not_exist"
        );
    }
}

#[test]
fn known_key_is_translated() {
    let (key, _) = TRANSLATIONS
        .entries()
        .next()
        .expect("translations not empty");
    for lang in ALL_LANGUAGES {
        let text = translate(key, lang);
        assert!(!text.is_empty());
        assert_ne!(text, *key, "'{key}' untranslated for {lang}");
    }
}

#[test]
fn every_key_has_every_language() {
    let mut missing = Vec::new();
    for (key, langs) in TRANSLATIONS.entries() {
        for lang in ALL_LANGUAGES {
            if !langs.contains_key(lang.code().0) {
                missing.push(format!("{key} [{lang}]"));
            }
        }
    }
    assert!(missing.is_empty(), "Missing translations: {missing:?}");
}

#[test]
fn replace_single_placeholder() {
    assert_eq!(
        translate_and_replace("imported {} sessions", &["3"], Languages::English),
        "imported 3 sessions"
    );
}

#[test]
fn replace_multiple_placeholders_in_order() {
    assert_eq!(
        translate_and_replace("{} of {}", &["1", "2"], Languages::English),
        "1 of 2"
    );
}

#[test]
fn language_codes() {
    assert!(matches!(Languages::from("es"), Languages::Spanish));
    assert!(matches!(Languages::from(" en "), Languages::English));
    assert!(matches!(Languages::from("fr"), Languages::English));
    assert!(matches!(
        Languages::from_name("Spanish"),
        Languages::Spanish
    ));
    assert!(matches!(
        Languages::from_name("English"),
        Languages::English
    ));

    for lang in ALL_LANGUAGES {
        assert!(matches!(
            (Languages::from(&lang.to_string()), lang),
            (Languages::Spanish, Languages::Spanish) | (Languages::English, Languages::English)
        ));
    }
}

#[test]
fn catalog_files_are_loaded() {
    assert_eq!(
        translate("exercise_13_65535", Languages::Spanish),
        "Hiperextensión"
    );
    assert_eq!(
        translate("category_13", Languages::English),
        "Hyperextension"
    );
    assert_eq!(translate("sport_10", Languages::English), "Training");
    assert_eq!(translate("sport_10_20", Languages::English), "Strength");
}

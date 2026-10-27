use phf_codegen::Map as PhfMap;
use std::fmt::Write as _;
use std::path::Path;
use std::{
    collections::HashMap,
    env,
    fs::{self},
    path::PathBuf,
};

fn main() {
    tauri_build::build();
    generate_translations_file();
}

/// Reads `resources/translations/translations.yaml` and `resources/translations/catalog/{exercises,categories,muscles}.yaml` and generates a static `phf::Map` of translations at `$OUT_DIR/translations_map.rs`.
fn generate_translations_file() {
    let translations_file = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .unwrap()
        .join("resources")
        .join("translations")
        .join("translations.yaml");
    println!("cargo:rerun-if-changed={}", translations_file.display());

    let catalog_file = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .unwrap()
        .join("resources")
        .join("translations")
        .join("catalog")
        .join("exercises.yaml");
    println!("cargo:rerun-if-changed={}", catalog_file.display());

    let categories_file = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .unwrap()
        .join("resources")
        .join("translations")
        .join("catalog")
        .join("categories.yaml");
    println!("cargo:rerun-if-changed={}", categories_file.display());

    let muscles_file = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap())
        .parent()
        .unwrap()
        .join("resources")
        .join("translations")
        .join("catalog")
        .join("muscles.yaml");
    println!("cargo:rerun-if-changed={}", muscles_file.display());

    let mut yaml_str = fs::read_to_string(&translations_file)
        .unwrap_or_else(|e| panic!("Could not read {}: {}", translations_file.display(), e));

    let mut translation_map =
        noyalib::from_str::<HashMap<String, HashMap<String, String>>>(&yaml_str)
            .unwrap_or_else(|e| panic!("Error parsing translations file: {}", e));

    yaml_str = fs::read_to_string(&catalog_file)
        .unwrap_or_else(|e| panic!("Could not read {}: {}", catalog_file.display(), e));

    let catalog_map = noyalib::from_str::<HashMap<String, HashMap<String, String>>>(&yaml_str)
        .unwrap_or_else(|e| panic!("Error parsing translations file: {}", e));

    translation_map.extend(catalog_map);

    yaml_str = fs::read_to_string(&categories_file)
        .unwrap_or_else(|e| panic!("Could not read {}: {}", categories_file.display(), e));

    let categories_map = noyalib::from_str::<HashMap<String, HashMap<String, String>>>(&yaml_str)
        .unwrap_or_else(|e| panic!("Error parsing translations file: {}", e));

    translation_map.extend(categories_map);

    yaml_str = fs::read_to_string(&muscles_file)
        .unwrap_or_else(|e| panic!("Could not read {}: {}", muscles_file.display(), e));

    let muscles_map = noyalib::from_str::<HashMap<String, HashMap<String, String>>>(&yaml_str)
        .unwrap_or_else(|e| panic!("Error parsing translations file: {}", e));

    translation_map.extend(muscles_map);

    let mut inner_codes: Vec<(String, String)> = Vec::new();
    for (key, translations) in &translation_map {
        let mut inner_map: PhfMap<&str> = PhfMap::new();
        for (lang, text) in translations {
            inner_map.entry(lang.as_str(), &format!("{:?}", text).clone());
        }
        inner_codes.push((key.clone(), inner_map.build().to_string()));
    }
    let mut outer_map: PhfMap<&str> = PhfMap::new();
    for (key, inner_code) in &inner_codes {
        outer_map.entry(key.as_str(), inner_code);
    }

    let mut out = String::new();
    out.push_str("// Automatically generated file, don't edit manually.\n");
    writeln!(
        out,
        "pub static TRANSLATIONS: phf::Map<&'static str, phf::Map<&'static str, &'static str>> = {};",
        outer_map.build()
    )
    .unwrap();

    let out_dir = env::var("OUT_DIR").unwrap();
    let dest_path = Path::new(&out_dir).join("translations_map.rs");
    fs::write(&dest_path, out).unwrap_or_else(|e| panic!("Could not write generated code: {}", e));
}

use std::fs;
use std::path::PathBuf;

use packtrans_glossary_wasm::test_fixtures;

fn main() -> anyhow::Result<()> {
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../wasm/fixtures");
    fs::create_dir_all(&out_dir)?;

    for lang in ["fr_fr", "zh_cn"] {
        let bytes = test_fixtures::build_index_zip(lang);
        let path = out_dir.join(format!("{lang}.zip"));
        fs::write(&path, bytes)?;
        println!("wrote {}", path.display());
    }

    Ok(())
}

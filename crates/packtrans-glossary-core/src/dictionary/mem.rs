use std::collections::HashMap;

use anyhow::{Result, anyhow};
use lindera_dictionary::LinderaResult;
use lindera_dictionary::decompress::{CompressedData, decompress};
use lindera_dictionary::dictionary::Dictionary;
use lindera_dictionary::dictionary::character_definition::CharacterDefinition;
use lindera_dictionary::dictionary::connection_cost_matrix::ConnectionCostMatrix;
use lindera_dictionary::dictionary::metadata::Metadata;
use lindera_dictionary::dictionary::prefix_dictionary::PrefixDictionary;
use lindera_dictionary::dictionary::unknown_dictionary::UnknownDictionary;
use lindera_dictionary::error::LinderaErrorKind;
use rkyv::util::AlignedVec;

use crate::archive::{dictionary_prefix_from_files, extract_zip_to_map};

pub fn load_dictionary_from_zip(zip_bytes: &[u8]) -> Result<Dictionary> {
    let files = extract_zip_to_map(zip_bytes)?;
    let prefix = dictionary_prefix_from_files(&files)?;
    load_dictionary_from_files(&files, &prefix).map_err(|e| anyhow!("{e}"))
}

fn load_dictionary_from_files(
    files: &HashMap<String, Vec<u8>>,
    prefix: &str,
) -> LinderaResult<Dictionary> {
    let read = |name: &str| -> LinderaResult<Vec<u8>> {
        let key = format!("{prefix}{name}");
        files.get(&key).cloned().ok_or_else(|| {
            LinderaErrorKind::Io.with_error(anyhow!("missing dictionary file: {key}"))
        })
    };

    Ok(Dictionary {
        metadata: load_metadata(&read)?,
        character_definition: load_character_definition(&read)?,
        connection_cost_matrix: load_connection_cost_matrix(&read)?,
        prefix_dictionary: load_prefix_dictionary(&read)?,
        unknown_dictionary: load_unknown_dictionary(&read)?,
    })
}

fn load_metadata(read: &dyn Fn(&str) -> LinderaResult<Vec<u8>>) -> LinderaResult<Metadata> {
    let data = read("metadata.json")?;
    serde_json::from_slice(&data).map_err(|err| {
        LinderaErrorKind::Deserialize
            .with_error(anyhow!(err))
            .add_context("Failed to deserialize metadata.json file")
    })
}

fn load_character_definition(
    read: &dyn Fn(&str) -> LinderaResult<Vec<u8>>,
) -> LinderaResult<CharacterDefinition> {
    let raw_data = maybe_decompress(read("char_def.bin")?)?;
    let mut aligned_data = AlignedVec::<16>::new();
    aligned_data.extend_from_slice(&raw_data);
    CharacterDefinition::load(&aligned_data)
}

fn load_connection_cost_matrix(
    read: &dyn Fn(&str) -> LinderaResult<Vec<u8>>,
) -> LinderaResult<ConnectionCostMatrix> {
    Ok(ConnectionCostMatrix::load(maybe_decompress(read(
        "matrix.mtx",
    )?)?))
}

fn load_prefix_dictionary(
    read: &dyn Fn(&str) -> LinderaResult<Vec<u8>>,
) -> LinderaResult<PrefixDictionary> {
    Ok(PrefixDictionary::load(
        maybe_decompress(read("dict.da")?)?,
        maybe_decompress(read("dict.vals")?)?,
        maybe_decompress(read("dict.wordsidx")?)?,
        maybe_decompress(read("dict.words")?)?,
        true,
    ))
}

fn load_unknown_dictionary(
    read: &dyn Fn(&str) -> LinderaResult<Vec<u8>>,
) -> LinderaResult<UnknownDictionary> {
    let raw_data = maybe_decompress(read("unk.bin")?)?;
    let mut aligned_data = AlignedVec::<16>::new();
    aligned_data.extend_from_slice(&raw_data);
    UnknownDictionary::load(&aligned_data)
}

fn maybe_decompress(data: Vec<u8>) -> LinderaResult<Vec<u8>> {
    let mut aligned = AlignedVec::<16>::new();
    aligned.extend_from_slice(&data);
    if let Ok(compressed_data) = rkyv::from_bytes::<CompressedData, rkyv::rancor::Error>(&aligned) {
        return decompress(compressed_data).map_err(|err| {
            LinderaErrorKind::Compression
                .with_error(anyhow!(err))
                .add_context("Failed to decompress dictionary component")
        });
    }
    Ok(data)
}

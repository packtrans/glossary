//! Shared in-memory Tantivy index zips for unit tests and Node.js WASM tests.

use std::collections::HashMap;
use std::io::{self, BufWriter, Cursor, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tantivy::directory::{
    AntiCallToken, Directory, DirectoryLock, FileHandle, Lock, RamDirectory, TerminatingWrite,
    WatchCallback, WatchHandle, WritePtr,
    error::{DeleteError, LockError, OpenReadError, OpenWriteError},
};
use tantivy::schema::{Schema, STORED, TEXT};
use tantivy::{Index, IndexSettings, TantivyDocument};
use zip::write::SimpleFileOptions;
use zip::ZipArchive;
use zip::ZipWriter;

#[derive(Clone, Copy)]
struct FixtureFields {
    mod_id: tantivy::schema::Field,
    key: tantivy::schema::Field,
    source_lang: tantivy::schema::Field,
    source_text: tantivy::schema::Field,
    target_lang: tantivy::schema::Field,
    target_text: tantivy::schema::Field,
}

/// Builds a minimal glossary index zip for the given language code.
pub fn build_index_zip(lang: &str) -> Vec<u8> {
    let mut builder = Schema::builder();
    let fields = FixtureFields {
        mod_id: builder.add_text_field("mod_id", STORED),
        key: builder.add_text_field("key", STORED),
        source_lang: builder.add_text_field("source_lang", STORED),
        source_text: builder.add_text_field("source_text", TEXT | STORED),
        target_lang: builder.add_text_field("target_lang", STORED),
        target_text: builder.add_text_field("target_text", TEXT | STORED),
    };
    let schema = builder.build();
    let ram_dir = RamDirectory::create();
    let index = Index::create(ram_dir.clone(), schema, IndexSettings::default()).unwrap();
    let mut writer = index.writer(50_000_000).unwrap();
    let mut doc = TantivyDocument::default();
    doc.add_text(fields.mod_id, "farmersdelight");
    doc.add_text(fields.key, "block.farmersdelight.cooking_pot");
    doc.add_text(fields.source_lang, "en_us");
    doc.add_text(fields.source_text, "Cooking Pot");
    doc.add_text(fields.target_lang, lang);
    doc.add_text(
        fields.target_text,
        if lang == "zh_cn" { "厨锅" } else { "Marmite" },
    );
    writer.add_document(doc).unwrap();
    writer.commit().unwrap();
    drop(writer);

    let recording_dir = RecordingDirectory::default();
    ram_dir.persist(&recording_dir).unwrap();
    zip_index_files(&recording_dir.files(), lang)
}

/// Builds an index zip that also contains a Tantivy lock file entry.
pub fn build_index_zip_with_lock(lang: &str) -> Vec<u8> {
    let zip_bytes = build_index_zip(lang);
    add_file_to_zip(zip_bytes, &format!("{lang}/.tantivy-meta.lock"), &[])
}

fn add_file_to_zip(mut zip_bytes: Vec<u8>, entry_name: &str, data: &[u8]) -> Vec<u8> {
    let cursor = Cursor::new(std::mem::take(&mut zip_bytes));
    let mut archive = ZipArchive::new(cursor).unwrap();
    let output = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(output);

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let existing_name = entry.name().to_string();
        writer
            .start_file(&existing_name, SimpleFileOptions::default())
            .unwrap();
        io::copy(&mut entry, &mut writer).unwrap();
    }

    writer
        .start_file(entry_name, SimpleFileOptions::default())
        .unwrap();
    writer.write_all(data).unwrap();
    writer.finish().unwrap().into_inner()
}

fn zip_index_files(files: &HashMap<PathBuf, Vec<u8>>, lang: &str) -> Vec<u8> {
    let cursor = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(cursor);
    let mut paths = files.keys().collect::<Vec<_>>();
    paths.sort();
    for path in paths {
        let name = Path::new(lang)
            .join(path)
            .to_string_lossy()
            .replace('\\', "/");
        writer
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&files[path]).unwrap();
    }
    writer.finish().unwrap().into_inner()
}

#[derive(Clone, Debug, Default)]
struct RecordingDirectory {
    files: Arc<Mutex<HashMap<PathBuf, Vec<u8>>>>,
}

impl RecordingDirectory {
    fn files(&self) -> HashMap<PathBuf, Vec<u8>> {
        self.files.lock().unwrap().clone()
    }
}

impl Directory for RecordingDirectory {
    fn get_file_handle(
        &self,
        path: &Path,
    ) -> std::result::Result<Arc<dyn FileHandle>, OpenReadError> {
        panic!(
            "unexpected read from recording directory: {}",
            path.display()
        )
    }

    fn delete(&self, path: &Path) -> std::result::Result<(), DeleteError> {
        panic!(
            "unexpected delete from recording directory: {}",
            path.display()
        )
    }

    fn exists(&self, path: &Path) -> std::result::Result<bool, OpenReadError> {
        Ok(self.files.lock().unwrap().contains_key(path))
    }

    fn open_write(&self, path: &Path) -> std::result::Result<WritePtr, OpenWriteError> {
        Ok(BufWriter::new(Box::new(RecordingWriter {
            path: path.to_path_buf(),
            files: Arc::clone(&self.files),
            data: Vec::new(),
        })))
    }

    fn atomic_read(&self, path: &Path) -> std::result::Result<Vec<u8>, OpenReadError> {
        self.files
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| OpenReadError::FileDoesNotExist(path.to_path_buf()))
    }

    fn atomic_write(&self, path: &Path, data: &[u8]) -> io::Result<()> {
        self.files
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), data.to_vec());
        Ok(())
    }

    fn sync_directory(&self) -> io::Result<()> {
        Ok(())
    }

    fn acquire_lock(&self, _lock: &Lock) -> std::result::Result<DirectoryLock, LockError> {
        panic!("unexpected lock from recording directory")
    }

    fn watch(&self, _watch_callback: WatchCallback) -> tantivy::Result<WatchHandle> {
        panic!("unexpected watch from recording directory")
    }
}

struct RecordingWriter {
    path: PathBuf,
    files: Arc<Mutex<HashMap<PathBuf, Vec<u8>>>>,
    data: Vec<u8>,
}

impl Write for RecordingWriter {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.data.extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.files
            .lock()
            .unwrap()
            .insert(self.path.clone(), self.data.clone());
        Ok(())
    }
}

impl TerminatingWrite for RecordingWriter {
    fn terminate_ref(&mut self, _: AntiCallToken) -> io::Result<()> {
        self.flush()
    }
}

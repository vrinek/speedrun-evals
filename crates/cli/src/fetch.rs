//! Downloads model files into the cache and refuses any that do not match
//! the catalogue's SHA-256.

use crate::catalog::Model;
use sha2::{Digest, Sha256};
use std::{
    fs::{self, File},
    io::{self, Read, Write},
    path::{Path, PathBuf},
};

/// Returns the local path of the model, downloading it first if needed.
pub fn ensure(model: &Model, cache_dir: &Path) -> Result<PathBuf, String> {
    let path = cache_dir.join("models").join(format!("{}.gguf", model.id));
    // A file only gets this name after its hash was checked, so a cached file is trusted.
    if path.exists() {
        return Ok(path);
    }
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;
    eprintln!("Downloading {} ...", model.url);
    let response = ureq::get(&model.url).call().map_err(|e| format!("{}: {e}", model.url))?;
    write_verified(response.into_body().into_reader(), &path, &model.sha256)?;
    Ok(path)
}

/// Streams `source` to `dest`, keeping the file only if its SHA-256 is `expected`.
fn write_verified(mut source: impl Read, dest: &Path, expected: &str) -> Result<(), String> {
    let part = dest.with_extension("part");
    let result = (|| {
        let mut file = File::create(&part)?;
        let mut hasher = Sha256::new();
        let mut buffer = vec![0; 1 << 20];
        loop {
            let n = source.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
            file.write_all(&buffer[..n])?;
        }
        file.sync_all()?;
        io::Result::Ok(hasher.finalize().iter().map(|byte| format!("{byte:02x}")).collect::<String>())
    })();
    let outcome = match result {
        Ok(actual) if actual.eq_ignore_ascii_case(expected) => fs::rename(&part, dest).map_err(|e| e.to_string()),
        Ok(actual) => Err(format!("SHA-256 mismatch: expected {expected}, got {actual}")),
        Err(e) => Err(format!("download failed: {e}")),
    };
    if outcome.is_err() {
        let _ = fs::remove_file(&part);
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    // SHA-256 of the bytes "hello".
    const HELLO: &str = "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824";

    fn temp(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!("speedrun-test-{}-{name}", std::process::id()))
    }

    #[test]
    fn keeps_a_file_with_the_expected_hash() {
        let dest = temp("good");
        write_verified(&b"hello"[..], &dest, HELLO).unwrap();
        assert_eq!(fs::read(&dest).unwrap(), b"hello");
        fs::remove_file(dest).unwrap();
    }

    #[test]
    fn discards_a_file_with_the_wrong_hash() {
        let dest = temp("bad");
        let err = write_verified(&b"tampered"[..], &dest, HELLO).unwrap_err();
        assert!(err.contains("SHA-256"), "{err}");
        assert!(!dest.exists());
        assert!(!dest.with_extension("part").exists());
    }
}

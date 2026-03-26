#![cfg(test)]

use git2::Repository;

#[test]
fn curl() {
    curl::init();
}

#[test]
fn git2() {
    Repository::discover(".").unwrap();
}

#[test]
fn zstd() {
    let data: &[_] = include_bytes!("lib.rs");
    assert!(!data.is_empty());
    let encoded = zstd::encode_all(data, 3).unwrap();
    let decoded = zstd::decode_all(encoded.as_slice()).unwrap();
    assert_eq!(data, &decoded);
}

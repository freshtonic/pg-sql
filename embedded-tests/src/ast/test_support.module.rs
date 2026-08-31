#[cfg(test)]
pub(crate) mod test_support {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/embedded-tests/src/ast/test_support.body.rs"
    ));
}

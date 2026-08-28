recursa::tokens! {
    literals { Word<'input>(source) => r"[a-z]+" with crate::scan, }
}

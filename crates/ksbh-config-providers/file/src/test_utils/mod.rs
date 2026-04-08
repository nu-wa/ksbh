pub struct Context {
    pub tmp_file: tempfile::NamedTempFile,
}

impl Context {
    pub fn new(file_content: &str) -> Self {
        let tmp_file = tempfile::NamedTempFile::with_suffix(".yaml").unwrap();

        ::std::fs::write(tmp_file.path(), file_content).unwrap();

        Self { tmp_file }
    }
}

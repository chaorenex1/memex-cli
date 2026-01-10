#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesMode {
    Embed,
    Ref,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilesEncoding {
    Utf8,
    Base64,
    Auto,
}

#[derive(Debug, Clone)]
pub struct StdioTask {
    pub id: String,
    pub backend: String,
    pub workdir: String,
    pub model: Option<String>,
    pub model_provider: Option<String>,
    pub dependencies: Vec<String>,
    pub stream_format: String,
    pub timeout: Option<u64>,
    pub retry: Option<u32>,
    pub files: Vec<String>,
    pub files_mode: FilesMode,
    pub files_encoding: FilesEncoding,
    pub content: String,
}

#[derive(Debug, Clone)]
pub struct StdioRunOpts {
    pub stream_format: String,
    pub ascii: bool,
    pub verbose: bool,
    pub quiet: bool,
    pub capture_bytes: usize,
    pub resume_run_id: Option<String>,
    pub resume_context: Option<String>,
}

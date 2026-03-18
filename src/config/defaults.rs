pub const DEFAULT_TOKEN_BUDGET: u32 = 8000;
pub const DEFAULT_STALENESS_TTL_INDEXED: u64 = 86400;
pub const DEFAULT_STALENESS_TTL_TOOL_RETURNED: u64 = 3600;
pub const DEFAULT_STALENESS_TTL_AGENT_INFERRED: u64 = 1800;
pub const DEFAULT_DEBOUNCE_MS: u64 = 500;
pub const DEFAULT_MAX_FILE_SIZE_KB: u64 = 500;

pub const BUILTIN_IGNORE_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "__pycache__",
    "dist",
    "build",
    ".aesp",
    ".next",
    ".nuxt",
    "venv",
    ".venv",
    "env",
    ".tox",
    "coverage",
    ".nyc_output",
];

pub const BUILTIN_IGNORE_EXTENSIONS: &[&str] = &[
    "exe", "dll", "so", "dylib", "o", "obj", "pyc", "pyo", "class", "jar", "war", "ear", "png",
    "jpg", "jpeg", "gif", "bmp", "ico", "svg", "woff", "woff2", "ttf", "eot", "mp3", "mp4",
    "avi", "mov", "pdf", "zip", "tar", "gz", "rar", "7z", "lock",
];

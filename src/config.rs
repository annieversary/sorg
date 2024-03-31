use vfs::{MemoryFS, VfsPath};

#[derive(Clone)]
#[allow(dead_code)]
pub struct Config {
    pub build_path: VfsPath,
    pub static_path: VfsPath,
    pub templates_path: VfsPath,
    pub verbose: bool,
    pub release: bool,

    pub url: String,
    pub title: String,
    pub description: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            build_path: VfsPath::new(MemoryFS::new()),
            static_path: VfsPath::new(MemoryFS::new()),
            templates_path: VfsPath::new(MemoryFS::new()),

            verbose: false,
            release: false,

            url: Default::default(),
            title: Default::default(),
            description: Default::default(),
        }
    }
}

pub const TODO_KEYWORDS: TodoKeywords = TodoKeywords {
    todo: &["TODO", "PROGRESS", "WAITING", "MAYBE", "CANCELLED"],
    done: &["DONE", "READ"],
};

pub struct TodoKeywords {
    pub todo: &'static [&'static str],
    pub done: &'static [&'static str],
}

impl TodoKeywords {
    pub fn to_org_config(&self) -> (Vec<String>, Vec<String>) {
        (
            self.todo.iter().map(ToString::to_string).collect(),
            self.done.iter().map(ToString::to_string).collect(),
        )
    }
}

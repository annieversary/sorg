use std::path::PathBuf;

#[derive(Default, Clone)]
#[allow(dead_code)]
pub struct Config {
    pub build_path: PathBuf,
    pub static_path: PathBuf,
    pub templates_path: PathBuf,
    pub verbose: bool,
    pub release: bool,

    pub url: String,
    pub title: String,
    pub description: String,
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

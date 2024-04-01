use std::{collections::HashMap, path::PathBuf};

use color_eyre::{eyre::ContextCompat, Result};
use orgize::Org;
use vfs::{MemoryFS, VfsPath};

use crate::args::Args;

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct Config {
    pub root_folder: PathBuf,
    pub build_folder: PathBuf,
    pub templates_folder: PathBuf,

    // vfs
    pub build_path: VfsPath,
    pub static_path: VfsPath,
    pub templates_path: VfsPath,

    pub verbose: bool,
    pub release: bool,
    pub hotreloading: bool,

    pub preamble: HashMap<String, String>,
    pub url: String,
    pub title: String,
    pub description: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            root_folder: Default::default(),
            build_folder: Default::default(),
            templates_folder: Default::default(),

            build_path: VfsPath::new(MemoryFS::new()),
            static_path: VfsPath::new(MemoryFS::new()),
            templates_path: VfsPath::new(MemoryFS::new()),

            verbose: false,
            release: false,
            hotreloading: false,

            preamble: Default::default(),
            url: Default::default(),
            title: Default::default(),
            description: Default::default(),
        }
    }
}

impl Config {
    pub fn new(fs: &VfsPath, args: &Args, org: &Org<'_>) -> Result<Self> {
        let preamble = org
            .keywords()
            .map(|v| (v.key.as_ref(), v.value.as_ref()))
            .collect::<HashMap<_, _>>();

        // required in preamble
        let title = preamble
            .get("title")
            .context("Keyword 'title' was not provided")?;
        let description = preamble
            .get("description")
            .context("Keyword 'description' was not provided")?;

        let url = if args.is_release() {
            preamble
                .get("url")
                .context("Keyword 'url' was not provided")?
        } else {
            // TODO this will break if this port is already in use and we end up using a different one
            // we should probably start the server before rendering, so we can know what port we are using
            "http://localhost:1024"
        };

        // paths
        let static_path = fs
            .clone()
            .join(preamble.get("static").unwrap_or(&"static"))?;

        let build = preamble.get("build").unwrap_or(&"build");
        let build_path = fs.clone().join(build)?;
        let build_folder = {
            let mut path = args.root_folder();
            path.push(build);
            path
        };

        let templates = preamble.get("templates").unwrap_or(&"templates");
        let templates_path = fs.clone().join(templates)?;
        let templates_folder = {
            let mut path = args.root_folder();
            path.push(templates);
            path
        };

        let config = Self {
            root_folder: args.root_folder(),
            templates_folder,
            build_folder,

            build_path: build_path.clone(),
            static_path: static_path.clone(),
            templates_path: templates_path.clone(),

            verbose: args.verbose,
            release: args.is_release(),
            hotreloading: args.is_hotreloading(),

            preamble: preamble
                .iter()
                .map(|(a, b)| (a.to_string(), b.to_string()))
                .collect(),
            url: url.to_string(),
            title: title.to_string(),
            description: description.to_string(),
        };
        Ok(config)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fails_if_empty_preamble() {
        let source = r#"
* index
"#;

        let fs: VfsPath = MemoryFS::new().into();
        let args = Args::default();
        let org = Org::parse(source);

        let config = Config::new(&fs, &args, &org).unwrap_err();

        assert_eq!("Keyword 'title' was not provided", format!("{}", config))
    }

    #[test]
    fn can_parse() {
        let source = r#"
#+title: this is a title
#+description: this is a description
#+url: a url here
"#;

        let fs: VfsPath = MemoryFS::new().into();
        let args = Args::default();
        let org = Org::parse(source);

        let config = Config::new(&fs, &args, &org).unwrap();

        assert_eq!("this is a title", config.title);
        assert_eq!("this is a description", config.description);
        assert_eq!("a url here", config.url);
    }
}

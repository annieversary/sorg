use clap::Parser;
use color_eyre::{eyre::Context, Result};
use orgize::{Org, ParseConfig};
use std::{collections::HashMap, fs::File, io::Read, path::Path};
use tera::Tera;

mod context;
mod helpers;
mod page;
mod template;
mod tera_functions;

use page::*;

/// Generate static site out of a single Org-mode file
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to the blog org-mode file
    #[arg(default_value = "./blog.org")]
    blog: String,
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

    let path = args.blog;
    let mut f = File::open(&path).with_context(|| format!("{path} was not found"))?;
    let mut src = String::new();
    f.read_to_string(&mut src)
        .with_context(|| format!("failed to read {path}"))?;

    let todos = (
        vec![
            "TODO".to_string(),
            "PROGRESS".to_string(),
            "WAITING".to_string(),
            "MAYBE".to_string(),
            "CANCELLED".to_string(),
        ],
        vec!["DONE".to_string(), "READ".to_string()],
    );

    let org = Org::parse_custom(
        &src,
        &ParseConfig {
            todo_keywords: todos.clone(),
        },
    );
    let keywords = org
        .keywords()
        .map(|v| (v.key.as_ref(), v.value.as_ref()))
        .collect::<HashMap<_, _>>();

    let build_path = keywords.get("out").unwrap_or(&"build");
    let static_path = keywords.get("static").unwrap_or(&"static");
    let templates_path = keywords.get("templates").unwrap_or(&"templates");

    let config = Config {
        build_path: build_path.to_string(),
        static_path: static_path.to_string(),
        templates_path: templates_path.to_string(),
        verbose: args.verbose,
    };

    let doc = org.document();

    let first = doc.first_child(&org).unwrap();

    let tree = Page::parse_index(&org, first, &todos);

    if Path::new(build_path).exists() {
        std::fs::remove_dir_all(build_path).expect("couldn't remove existing build directory");
    }

    let mut tera = Tera::new(&format!("{templates_path}/*.html"))?;
    tera.register_function("get_pages", tera_functions::make_get_pages(&tree));

    tree.render(&tera, build_path, &config)?;

    std::process::Command::new("/bin/sh")
        .args(["-c", &format!("cp -r {static_path}/* {build_path}")])
        .output()
        .expect("failed to execute process");

    if config.verbose {
        println!("done");
    }

    Ok(())
}

pub type Keywords = (Vec<String>, Vec<String>);

#[derive(Default, Clone)]
#[allow(dead_code)]
pub struct Config {
    build_path: String,
    static_path: String,
    templates_path: String,
    verbose: bool,
}

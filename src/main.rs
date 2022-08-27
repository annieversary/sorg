use color_eyre::Result;
use orgize::{Org, ParseConfig};
use std::{fs::File, io::Read, path::Path};
use tera::Tera;

mod context;
mod helpers;
mod page;
mod template;

use page::*;

fn main() -> Result<()> {
    color_eyre::install()?;

    let build_path = "./build";

    let tera = Tera::new("templates/*.html")?;

    // TODO this file should come from args
    let mut f = File::open("./blog.org")?;
    let mut src = String::new();
    f.read_to_string(&mut src)?;

    let keywords = (
        vec![
            "TODO".to_string(),
            "PROGRESS".to_string(),
            "WAITING".to_string(),
            "MAYBE".to_string(),
            "CANCELLED".to_string(),
        ],
        vec!["DONE".to_string()],
    );

    let org = Org::parse_custom(
        &src,
        &ParseConfig {
            todo_keywords: keywords.clone(),
        },
    );
    let doc = org.document();

    let first = doc.first_child(&org).unwrap();

    let tree = Page::parse_index(&org, first, &keywords);

    if Path::new(build_path).exists() {
        std::fs::remove_dir_all(build_path).expect("couldn't remove existing build directory");
    }

    tree.render(&tera, build_path)?;

    std::process::Command::new("/bin/sh")
        .args(["-c", "cp -r static/* build"])
        .output()
        .expect("failed to execute process");
    println!("done");

    Ok(())
}

pub type Keywords = (Vec<String>, Vec<String>);

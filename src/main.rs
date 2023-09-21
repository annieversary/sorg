use clap::Parser;
use color_eyre::{
    eyre::{Context, ContextCompat},
    Result,
};
use notify_debouncer_mini::{new_debouncer, notify::*};
use orgize::{Org, ParseConfig};
use std::{collections::HashMap, path::Path, time::Duration};
use tera::Tera;

mod context;
mod helpers;
mod page;
mod render;
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
    #[arg(short, long)]
    watch: bool,
    #[arg(short, long)]
    serve: bool,
}

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

    let path = args.blog.clone();

    // read file once to get template directory
    let source =
        std::fs::read_to_string(&path).with_context(|| format!("Failed to read {path}"))?;
    let org = parse(&source)?;

    let keywords = org
        .keywords()
        .map(|v| (v.key.as_ref(), v.value.as_ref()))
        .collect::<HashMap<_, _>>();
    let templates_path = keywords
        .get("templates")
        .unwrap_or(&"templates")
        .to_string();
    let build_path = keywords.get("out").unwrap_or(&"build").to_string();

    // render once cause we always want to do that
    run(org, args.verbose)?;

    if args.watch {
        let mut watcher = new_debouncer(Duration::from_millis(100), move |res| match res {
            Ok(_event) => {
                fn cycle(path: &str, verbose: bool) -> Result<()> {
                    let source = std::fs::read_to_string(path)
                        .with_context(|| format!("Failed to read {path}"))?;
                    let org = parse(&source)?;
                    run(org, verbose)?;
                    Ok(())
                }
                if let Err(err) = cycle(&path, args.verbose) {
                    println!("Error occurred: {err}");
                }
            }
            Err(e) => println!("watch error: {:?}", e),
        })?;

        watcher
            .watcher()
            .watch(Path::new(&args.blog), RecursiveMode::Recursive)?;
        watcher
            .watcher()
            .watch(Path::new(&templates_path), RecursiveMode::Recursive)?;

        let server = file_serve::Server::new(&build_path);
        println!("Serving at http://{}", server.addr());

        server.serve().unwrap();
    } else if args.serve {
        let server = file_serve::Server::new(&build_path);
        println!("Serving at http://{}", server.addr());

        server.serve().unwrap();
    }

    Ok(())
}

fn todos() -> (Vec<String>, Vec<String>) {
    (
        vec![
            "TODO".to_string(),
            "PROGRESS".to_string(),
            "WAITING".to_string(),
            "MAYBE".to_string(),
            "CANCELLED".to_string(),
        ],
        vec!["DONE".to_string(), "READ".to_string()],
    )
}

fn parse(src: &str) -> Result<Org<'_>> {
    let org = Org::parse_custom(
        src,
        &ParseConfig {
            todo_keywords: todos(),
        },
    );

    Ok(org)
}

fn run(org: Org<'_>, verbose: bool) -> Result<()> {
    let keywords = org
        .keywords()
        .map(|v| (v.key.as_ref(), v.value.as_ref()))
        .collect::<HashMap<_, _>>();

    let build_path = keywords.get("out").unwrap_or(&"build");
    let static_path = keywords.get("static").unwrap_or(&"static");
    let templates_path = keywords.get("templates").unwrap_or(&"templates");
    let url = keywords
        .get("url")
        .context("Keyword 'url' was not provided")?;
    let title = keywords
        .get("title")
        .context("Keyword 'title' was not provided")?;
    let description = keywords
        .get("description")
        .context("Keyword 'description' was not provided")?;

    let config = Config {
        build_path: build_path.to_string(),
        static_path: static_path.to_string(),
        templates_path: templates_path.to_string(),
        verbose,

        url: url.to_string(),
        title: title.to_string(),
        description: description.to_string(),
    };

    let doc = org.document();

    let first = doc.first_child(&org).unwrap();

    let tree = Page::parse_index(&org, first, &todos(), "".to_string(), 0);

    if Path::new(build_path).exists() {
        std::fs::remove_dir_all(build_path).expect("couldn't remove existing build directory");
    }

    let mut tera = Tera::new(&format!("{templates_path}/*.html"))?;
    tera.register_function("get_pages", tera_functions::make_get_pages(&tree));

    tree.render(&tera, build_path, &config, &org)?;

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

    url: String,
    title: String,
    description: String,
}

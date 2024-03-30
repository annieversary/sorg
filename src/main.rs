use color_eyre::{
    eyre::{Context, ContextCompat},
    Result,
};
use notify_debouncer_mini::{new_debouncer, notify::*};
use orgize::{Org, ParseConfig};
use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
};
use tera::Tera;

mod context;
mod helpers;
mod hotreloading;
mod page;
mod render;
mod template;
mod tera_functions;

use page::*;

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = parse_args();

    let release = args.mode == SorgMode::Run;

    // read file once to get template directory
    let source = std::fs::read_to_string(&args.path)
        .with_context(|| format!("Failed to read {}", &args.path.to_string_lossy()))?;
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
    run(
        &args.path,
        org,
        args.verbose,
        release,
        args.mode == SorgMode::Watch,
    )?;

    if args.mode == SorgMode::Watch {
        let (_ws_thread, ws_tx) = hotreloading::init_websockets();

        let path = args.path.clone();
        let mut watcher = new_debouncer(Duration::from_millis(100), move |res| match res {
            Ok(_event) => {
                fn cycle(path: &Path, verbose: bool, release: bool) -> Result<()> {
                    let source = std::fs::read_to_string(path)
                        .with_context(|| format!("Failed to read {}", path.to_string_lossy()))?;
                    let org = parse(&source)?;
                    run(path, org, verbose, release, true)?;

                    Ok(())
                }
                if let Err(err) = cycle(&path, args.verbose, release) {
                    println!("Error occurred: {err}");
                } else {
                    // tell websocket to reload
                    ws_tx.send(()).unwrap();
                }
            }
            Err(e) => println!("watch error: {:?}", e),
        })?;

        watcher
            .watcher()
            .watch(Path::new(&args.path), RecursiveMode::Recursive)?;
        watcher
            .watcher()
            .watch(Path::new(&templates_path), RecursiveMode::Recursive)?;

        let server = file_serve::Server::new(&build_path);
        println!("Serving at http://{}", server.addr());

        server.serve().unwrap();
    } else if args.mode == SorgMode::Serve {
        let server = file_serve::Server::new(&build_path);
        println!("Serving at http://{}", server.addr());

        server.serve().unwrap();
    }

    Ok(())
}

#[derive(PartialEq, Eq, Debug)]
enum SorgMode {
    Run,
    Serve,
    Watch,
}

#[derive(Debug)]
struct Args {
    mode: SorgMode,
    path: PathBuf,
    verbose: bool,
}

fn parse_args() -> Args {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let verbose = args.iter().any(|s| s == "-v" || s == "--verbose");

    let args: Vec<_> = args
        .iter()
        .filter(|s| *s != "-v" && *s != "--verbose")
        .map(AsRef::as_ref)
        .collect();
    let slice = if args.len() >= 2 {
        &args[..2]
    } else {
        &args[..]
    };

    match slice {
        [] => Args {
            mode: SorgMode::Run,
            path: "./blog.org".into(),
            verbose,
        },
        ["watch"] => Args {
            mode: SorgMode::Watch,
            path: "./blog.org".into(),
            verbose,
        },
        ["serve"] => Args {
            mode: SorgMode::Serve,
            path: "./blog.org".into(),
            verbose,
        },
        [path] => Args {
            mode: SorgMode::Run,
            path: path.into(),
            verbose,
        },
        ["watch", path] => Args {
            mode: SorgMode::Watch,
            path: path.into(),
            verbose,
        },
        ["serve", path] => Args {
            mode: SorgMode::Serve,
            path: path.into(),
            verbose,
        },
        _ => panic!("unparsable input"),
    }
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

fn localized_path(path: &Path, file: &str) -> PathBuf {
    let mut path = path.to_path_buf();
    path.pop();
    path.push(file);
    path
}

fn run(path: &Path, org: Org<'_>, verbose: bool, release: bool, hotreloading: bool) -> Result<()> {
    let keywords = org
        .keywords()
        .map(|v| (v.key.as_ref(), v.value.as_ref()))
        .collect::<HashMap<_, _>>();

    let build_path = localized_path(path, keywords.get("out").unwrap_or(&"build"));
    let static_path = localized_path(path, keywords.get("static").unwrap_or(&"static"));
    let templates_path = localized_path(path, keywords.get("templates").unwrap_or(&"templates"));
    let url = if release {
        keywords
            .get("url")
            .context("Keyword 'url' was not provided")?
    } else {
        // TODO this will break if this port is already in use and we end up using a different one
        // we should probably start the server before rendering, so we can know what port we are using
        "http://localhost:1024"
    };
    let title = keywords
        .get("title")
        .context("Keyword 'title' was not provided")?;
    let description = keywords
        .get("description")
        .context("Keyword 'description' was not provided")?;

    let config = Config {
        build_path: build_path.clone(),
        static_path: static_path.clone(),
        templates_path: templates_path.clone(),
        verbose,
        release,

        url: url.to_string(),
        title: title.to_string(),
        description: description.to_string(),
    };

    let doc = org.document();

    let first = doc.first_child(&org).unwrap();

    let tree = Page::parse_index(&org, first, &todos(), "".to_string(), 0, release);

    if build_path.exists() {
        std::fs::remove_dir_all(&build_path).expect("couldn't remove existing build directory");
    }

    let mut tera = Tera::new(&format!("{}/*.html", templates_path.to_string_lossy()))?;
    tera.register_function("get_pages", tera_functions::make_get_pages(&tree));

    tree.render(&tera, build_path.clone(), &config, &org, hotreloading)?;

    std::process::Command::new("/bin/sh")
        .args([
            "-c",
            &format!(
                "cp -r {}/* {}",
                static_path.to_string_lossy(),
                build_path.to_string_lossy()
            ),
        ])
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
    build_path: PathBuf,
    static_path: PathBuf,
    templates_path: PathBuf,
    verbose: bool,
    release: bool,

    url: String,
    title: String,
    description: String,
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

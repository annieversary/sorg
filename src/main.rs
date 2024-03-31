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

mod args;
mod config;
mod context;
mod helpers;
mod hotreloading;
mod page;
mod render;
mod template;
mod tera_functions;

use args::{Args, SorgMode};
use config::{Config, TODO_KEYWORDS};
use page::*;

fn main() -> Result<()> {
    color_eyre::install()?;

    let args = Args::parse();

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

    match args.mode {
        SorgMode::Run => build_files(&args.path, org, args.verbose, args.is_release(), false)?,
        SorgMode::Serve => {
            build_files(&args.path, org, args.verbose, args.is_release(), false)?;

            let server = file_serve::Server::new(&build_path);
            println!("Serving at http://{}", server.addr());

            server.serve().unwrap();
        }
        SorgMode::Watch => {
            build_files(&args.path, org, args.verbose, args.is_release(), true)?;

            let (_ws_thread, ws_tx) = hotreloading::init_websockets();

            let path = args.path.clone();
            let release = args.is_release();
            let mut watcher = new_debouncer(Duration::from_millis(100), move |res| match res {
                Ok(_event) => {
                    fn cycle(path: &Path, verbose: bool, release: bool) -> Result<()> {
                        let source = std::fs::read_to_string(path).with_context(|| {
                            format!("Failed to read {}", path.to_string_lossy())
                        })?;
                        let org = parse(&source)?;
                        build_files(path, org, verbose, release, true)?;

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
        }
        SorgMode::Folders => todo!(),
    }

    Ok(())
}

fn parse(src: &str) -> Result<Org<'_>> {
    let org = Org::parse_custom(
        src,
        &ParseConfig {
            todo_keywords: TODO_KEYWORDS.to_org_config(),
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

fn build_files(
    path: &Path,
    org: Org<'_>,
    verbose: bool,
    release: bool,
    hotreloading: bool,
) -> Result<()> {
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

    let tree = Page::parse_index(&org, first, &TODO_KEYWORDS, "".to_string(), 0, release);

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

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
use vfs::{PhysicalFS, VfsPath};

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

    let args = Args::parse()?;

    let fs: VfsPath = PhysicalFS::new(args.root_folder()).into();

    let source = fs
        .join(args.file_name()?)?
        .read_to_string()
        .with_context(|| "Failed to read file")?;

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
        SorgMode::Run => build_files(
            &fs,
            args.root_folder(),
            org,
            args.verbose,
            args.is_release(),
            false,
        )?,
        SorgMode::Serve => {
            build_files(
                &fs,
                args.root_folder(),
                org,
                args.verbose,
                args.is_release(),
                false,
            )?;

            let folder_path = fs.join(build_path)?;
            let server = file_serve::Server::new(folder_path.as_str());
            println!("Serving at http://{}", server.addr());

            server.serve().unwrap();
        }
        SorgMode::Watch => {
            build_files(
                &fs,
                args.root_folder(),
                org,
                args.verbose,
                args.is_release(),
                true,
            )?;

            let (_ws_thread, ws_tx) = hotreloading::init_websockets();

            let release = args.is_release();
            let fs = fs.clone().join(args.file_name()?)?;
            let root_folder = args.root_folder();
            let mut watcher = new_debouncer(Duration::from_millis(100), move |res| match res {
                Ok(_event) => {
                    fn cycle(
                        fs: &VfsPath,
                        root_folder: PathBuf,
                        verbose: bool,
                        release: bool,
                    ) -> Result<()> {
                        let source = fs.read_to_string().with_context(|| "Failed to read file")?;
                        let org = parse(&source)?;
                        build_files(fs, root_folder, org, verbose, release, true)?;

                        Ok(())
                    }
                    if let Err(err) = cycle(&fs, root_folder.clone(), args.verbose, release) {
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

fn build_files(
    fs: &VfsPath,
    root_path: PathBuf,
    org: Org<'_>,
    verbose: bool,
    release: bool,
    hotreloading: bool,
) -> Result<()> {
    let keywords = org
        .keywords()
        .map(|v| (v.key.as_ref(), v.value.as_ref()))
        .collect::<HashMap<_, _>>();

    let build_path = fs.clone().join(keywords.get("out").unwrap_or(&"build"))?;
    let static_path = fs
        .clone()
        .join(keywords.get("static").unwrap_or(&"static"))?;
    let template_folder = keywords.get("templates").unwrap_or(&"templates");
    let templates_path = fs.clone().join(template_folder)?;
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

    if build_path.exists()? {
        build_path
            .remove_dir_all()
            .with_context(|| "Couldn't clear build directory")?;
    }

    static_path
        .copy_dir(&build_path)
        .with_context(|| "Failed to copy static folder into build folder")?;

    let mut template_folder_path = root_path;
    template_folder_path.push(template_folder);
    let mut tera = Tera::new(&format!(
        "{}/*.html",
        template_folder_path.to_string_lossy()
    ))?;
    tera.register_function("get_pages", tera_functions::make_get_pages(&tree));

    tree.render(&tera, build_path.clone(), &config, &org, hotreloading)?;

    if config.verbose {
        println!("done");
    }

    Ok(())
}

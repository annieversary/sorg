use color_eyre::{eyre::Context, Result};
use notify_debouncer_mini::{new_debouncer, notify::*};
use orgize::{Org, ParseConfig};
use std::{path::Path, time::Duration};
use tera::Tera;
use tera_functions::make_tera;
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
    let config = Config::new(&fs, &args, &org)?;
    let tera = make_tera(&config)?;

    match args.mode {
        SorgMode::Run => build_files(&config, org, tera)?,
        SorgMode::Serve => {
            build_files(&config, org, tera)?;

            let server = file_serve::Server::new(&config.build_folder);
            println!("Serving at http://{}", server.addr());

            server.serve().unwrap();
        }
        SorgMode::Watch => {
            build_files(&config, org, tera)?;

            let (_ws_thread, ws_tx) = hotreloading::init_websockets();

            let a = args.clone();
            let mut watcher = new_debouncer(Duration::from_millis(100), move |res| match res {
                Ok(_event) => {
                    fn cycle(fs: &VfsPath, args: &Args) -> Result<()> {
                        let source = fs
                            .join(args.file_name()?)?
                            .read_to_string()
                            .with_context(|| "Failed to read file")?;

                        let org = parse(&source)?;
                        let config = Config::new(fs, args, &org)?;
                        let tera = make_tera(&config)?;

                        build_files(&config, org, tera)?;

                        Ok(())
                    }
                    if let Err(err) = cycle(&fs, &a) {
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
                .watch(&config.templates_folder, RecursiveMode::Recursive)?;

            let server = file_serve::Server::new(&config.build_folder);
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

fn build_files(config: &Config, org: Org<'_>, mut tera: Tera) -> Result<()> {
    let doc = org.document();

    let first = doc.first_child(&org).unwrap();

    let tree = Page::parse_index(
        &org,
        first,
        &TODO_KEYWORDS,
        "".to_string(),
        0,
        config.release,
    );

    if config.build_path.exists()? {
        config
            .build_path
            .remove_dir_all()
            .with_context(|| "Couldn't clear build directory")?;
    }

    config
        .static_path
        .copy_dir(&config.build_path)
        .with_context(|| "Failed to copy static folder into build folder")?;

    tera.register_function("get_pages", tera_functions::make_get_pages(&tree));

    tree.render(
        &tera,
        config.build_path.clone(),
        config,
        &org,
        config.hotreloading,
    )?;

    if config.verbose {
        println!("done");
    }

    Ok(())
}

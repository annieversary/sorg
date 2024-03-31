use std::path::PathBuf;

#[derive(Debug)]
pub struct Args {
    pub mode: SorgMode,
    pub path: PathBuf,
    pub verbose: bool,
}

#[derive(PartialEq, Eq, Debug)]
pub enum SorgMode {
    /// Basic HTML generation from the provided file
    Run,
    /// Generate HTML and start server
    Serve,
    /// Generate HTML, start server, and watch for changes
    Watch,
    /// Generate folders in `static` for each node in the tree
    Folders,
}

impl Args {
    pub fn parse() -> Self {
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
            ["folders"] => Args {
                mode: SorgMode::Folders,
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
            ["folders", path] => Args {
                mode: SorgMode::Folders,
                path: path.into(),
                verbose,
            },
            _ => panic!("unparsable input"),
        }
    }

    pub fn is_release(&self) -> bool {
        self.mode == SorgMode::Run
    }
}

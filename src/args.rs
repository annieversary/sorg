use std::path::PathBuf;

use color_eyre::{
    eyre::{eyre, Error},
    Result,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Args {
    pub mode: SorgMode,
    pub path: PathBuf,
    pub verbose: bool,
}

#[derive(PartialEq, Eq, Debug, Clone, Default)]
pub enum SorgMode {
    /// Basic HTML generation from the provided file
    #[default]
    Run,
    /// Generate HTML and start server
    Serve,
    /// Generate HTML, start server, and watch for changes
    Watch,
    /// Generate folders in `static` for each node in the tree
    Folders,
    // TODO add option for Folders to create empty `.gitignore` files
}

impl TryFrom<&str> for SorgMode {
    type Error = Error;

    fn try_from(value: &str) -> std::result::Result<Self, Self::Error> {
        let v = match value {
            "run" => Self::Run,
            "serve" => Self::Serve,
            "watch" => Self::Watch,
            "folders" => Self::Folders,
            _ => return Err(eyre!("Unrecognized argument {value}")),
        };
        Ok(v)
    }
}

impl Args {
    pub fn from_env() -> Result<Self> {
        Self::parse(std::env::args().skip(1))
    }

    pub fn parse(args: impl Iterator<Item = impl ToString>) -> Result<Self> {
        let (args, argv) = argmap::parse(args);

        let verbose = argv.contains_key("v") || argv.contains_key("verbose");

        let (mode, path) = match &args[..] {
            [] => (SorgMode::Run, PathBuf::from("./blog.org")),
            [arg] => match SorgMode::try_from(arg.as_str()) {
                Ok(mode) => (mode, "./blog.org".into()),
                Err(_) => (SorgMode::Run, arg.into()),
            },
            [mode, path] => (SorgMode::try_from(mode.as_str())?, path.into()),
            _ => return Err(eyre!("Too many arguments")),
        };

        Ok(Args {
            mode,
            path,
            verbose,
        })
    }

    pub fn root_folder(&self) -> PathBuf {
        let mut path = self.path.to_path_buf();
        path.pop();
        path
    }

    pub fn file_name(&self) -> Result<&str> {
        self.path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| eyre!("No filename found"))
    }

    pub fn is_release(&self) -> bool {
        self.mode == SorgMode::Run
    }

    pub fn is_hotreloading(&self) -> bool {
        self.mode == SorgMode::Watch
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    macro_rules! test {
        ($args:expr, $solution:expr $(,)?) => {
            let args: &[&str] = &$args;
            let args = Args::parse(args.iter())?;
            assert_eq!($solution, args);
        };
    }

    #[test]
    fn parse() -> Result<()> {
        test!(
            [],
            Args {
                mode: SorgMode::Run,
                path: PathBuf::from("./blog.org"),
                verbose: false,
            },
        );
        test!(
            ["watch", "hey.org", "-v"],
            Args {
                mode: SorgMode::Watch,
                path: PathBuf::from("hey.org"),
                verbose: true,
            },
        );

        Ok(())
    }
}

use color_eyre::Report;
use std::{fs::File, io::Read};
use tree_sitter::{Language, Parser};

extern "C" {
    fn tree_sitter_org() -> Language;
}

fn main() -> Result<(), Report> {
    let mut parser = Parser::new();
    let language = unsafe { tree_sitter_org() };
    // let language = tree_sitter_org::language();
    parser.set_language(language).unwrap();

    let mut f = File::open("./test.org")?;
    let mut src = String::new();
    f.read_to_string(&mut src)?;

    let tree = parser.parse(&src.as_bytes(), None).unwrap();
    let root_node = tree.root_node();

    dbg!(root_node.utf8_text(&src.as_bytes()));

    Ok(())
}

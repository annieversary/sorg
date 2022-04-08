use std::path::PathBuf;

fn main() {
    let src_dir: PathBuf = ["tree-sitter-org", "src"].iter().collect();

    cc::Build::new()
        .cpp(false)
        .include(&src_dir)
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .flag_if_supported("-Wno-trigraphs")
        .file(&src_dir.join("parser.c"))
        .compile("tree_sitter_org_parser");
    // println!("cargo:rerun-if-changed={}", parser_path.to_str().unwrap());

    cc::Build::new()
        .cpp(true)
        .include(&src_dir)
        .flag("-xc++")
        .include(&src_dir)
        .flag_if_supported("-Wno-unused-parameter")
        .flag_if_supported("-Wno-unused-but-set-variable")
        .file(&src_dir.join("scanner.cc"))
        .compile("tree_sitter_org_scanner");
    // println!("cargo:rerun-if-changed={}", scanner_path.to_str().unwrap());
}

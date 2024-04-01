use color_eyre::eyre::Result;
use orgize::Org;
use vfs::VfsPath;

use crate::{
    config::TODO_KEYWORDS,
    page::{Page, PageEnum},
};

pub fn generate_folders(static_path: VfsPath, org: Org<'_>) -> Result<()> {
    let page = Page::parse_index(
        &org,
        org.document().first_child(&org).unwrap(),
        &TODO_KEYWORDS,
        "".to_string(),
        0,
        false,
    );

    generate_folder_for_page(static_path, &page)
}

fn generate_folder_for_page(path: VfsPath, page: &Page<'_>) -> Result<()> {
    let path = if page.info.slug == "index" {
        path
    } else {
        path.join(&page.info.slug)?
    };

    path.create_dir_all()?;

    if let PageEnum::Index { children } = &page.page {
        for page in children.values() {
            generate_folder_for_page(path.clone(), page)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use vfs::MemoryFS;

    use super::*;

    #[test]
    fn fails_if_empty_preamble() -> Result<()> {
        let source = r#"
* index
** first child
*** grandchild
** second page
"#;

        let fs: VfsPath = MemoryFS::new().into();
        let org = Org::parse(source);

        generate_folders(fs.clone(), org).unwrap();

        assert!(fs.join("first-child")?.exists()?);
        assert!(fs.join("first-child")?.join("grandchild")?.exists()?);
        assert!(fs.join("second-page")?.exists()?);

        Ok(())
    }
}

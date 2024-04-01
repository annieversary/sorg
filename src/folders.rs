use color_eyre::eyre::Result;
use orgize::Org;
use vfs::VfsPath;

use crate::{
    config::TODO_KEYWORDS,
    page::{Page, PageEnum},
};

pub fn generate_folders(
    static_path: VfsPath,
    org: Org<'_>,
    generate_gitignore: bool,
) -> Result<()> {
    let page = Page::parse_index(
        &org,
        org.document().first_child(&org).unwrap(),
        &TODO_KEYWORDS,
        "".to_string(),
        0,
        false,
    );

    generate_folder_for_page(static_path, &page, generate_gitignore)
}

fn generate_folder_for_page(
    path: VfsPath,
    page: &Page<'_>,
    generate_gitignore: bool,
) -> Result<()> {
    let path = if page.info.slug == "index" {
        path
    } else {
        path.join(&page.info.slug)?
    };

    path.create_dir_all()?;

    if generate_gitignore {
        let gitignore = path.join(".gitignore")?;
        if !gitignore.exists()? {
            gitignore.create_file()?;
        }
    }

    if let PageEnum::Index { children } = &page.page {
        for page in children.values() {
            generate_folder_for_page(path.clone(), page, generate_gitignore)?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use vfs::MemoryFS;

    use super::*;

    #[test]
    fn create_folders() -> Result<()> {
        let source = r#"
* index
** first child
*** grandchild
** second page
"#;

        let fs: VfsPath = MemoryFS::new().into();
        let org = Org::parse(source);

        generate_folders(fs.clone(), org, false).unwrap();

        assert!(fs.join("first-child")?.exists()?);
        assert!(fs.join("first-child")?.is_dir()?);
        assert!(fs.join("first-child")?.join("grandchild")?.exists()?);
        assert!(fs.join("first-child")?.join("grandchild")?.is_dir()?);
        assert!(fs.join("second-page")?.exists()?);

        Ok(())
    }

    #[test]
    fn creates_gitignores() -> Result<()> {
        let source = r#"
* index
** one
*** two
"#;

        let fs: VfsPath = MemoryFS::new().into();
        let org = Org::parse(source);

        let gitignore = fs.join("one")?.join("two")?.join(".gitignore")?;
        assert!(!gitignore.exists()?);

        generate_folders(fs.clone(), org, true).unwrap();

        // file exists and is empty
        assert!(gitignore.exists()?);
        assert!(gitignore.is_file()?);
        assert_eq!("", gitignore.read_to_string()?);

        Ok(())
    }

    #[test]
    fn doesnt_overwrite_gitignores() -> Result<()> {
        let source = r#"
* index
** one
*** two
"#;

        let fs: VfsPath = MemoryFS::new().into();
        let org = Org::parse(source);

        let path = fs.join("one")?.join("two")?;
        path.create_dir_all()?;
        let gitignore = path.join(".gitignore")?;
        gitignore.create_file()?.write_all("hiii :3".as_bytes())?;

        generate_folders(fs.clone(), org, true).unwrap();

        assert!(gitignore.exists()?);
        assert!(gitignore.is_file()?);
        assert_eq!("hiii :3", gitignore.read_to_string()?);

        Ok(())
    }
}

use orgize::{
    elements::{Datetime, Timestamp, Title},
    Headline, Org,
};
use slugmin::slugify;
use std::{borrow::Cow, collections::HashMap, path::PathBuf};

use crate::{config::TodoKeywords, helpers::parse_file_link};

#[derive(Debug)]
pub enum PageEnum<'a> {
    Index { children: HashMap<String, Page<'a>> },
    Post,
    OrgFile { path: PathBuf },
}

pub struct Page<'a> {
    pub headline: Headline,
    pub path: String,
    pub info: PageInfo<'a>,
    pub order: usize,
    pub page: PageEnum<'a>,
}

impl<'a> std::fmt::Debug for Page<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Page")
            .field("info", &self.info)
            .field("page", &self.page)
            .finish()
    }
}

impl<'a> Page<'a> {
    pub fn parse_index(
        org: &'a Org<'a>,
        headline: Headline,
        keywords: &TodoKeywords,
        mut path: String,
        order: usize,
        release: bool,
    ) -> Self {
        let title = headline.title(org);

        let info = PageInfo::new(title);

        if info.slug != "index" {
            path = format!("{path}/{}", info.slug);
        }
        let parent_is_posts = title.tags.contains(&Cow::Borrowed("posts"));

        let children = headline
            .children(org)
            .enumerate()
            .filter_map(|(order, headline)| {
                parse_child(
                    order,
                    headline,
                    org,
                    keywords,
                    release,
                    parent_is_posts,
                    &path,
                )
            })
            .map(|p| (p.info.slug.clone(), p))
            .collect();

        if path.is_empty() {
            path = "/".to_string();
        }

        Page {
            headline,
            page: PageEnum::Index { children },
            path,

            info,
            order,
        }
    }
}

fn parse_child<'a>(
    order: usize,
    headline: Headline,
    org: &'a Org<'a>,
    keywords: &TodoKeywords,
    release: bool,
    parent_is_posts: bool,
    path: &str,
) -> Option<Page<'a>> {
    let title = headline.title(org);

    // skip
    if title.tags.contains(&Cow::Borrowed("noexport")) {
        return None;
    }

    // if there's a keyword on this post, and it's in TODO/PROGRESS, we skip it
    if let Some(kw) = &title.keyword {
        if keywords.todo.contains(&kw.as_ref()) && (release || kw != "PROGRESS") {
            return None;
        }
    }

    // if this is doesnt have the `post` tag and parent is not `posts`, treat it as an index page
    let is_post = title.tags.contains(&Cow::Borrowed("post"));
    if !is_post && !parent_is_posts {
        return Some(Page::parse_index(
            org,
            headline,
            keywords,
            path.to_string(),
            order,
            release,
        ));
    }

    let info = PageInfo::new(title);

    // check if it's a linked file
    let file_prop = title
        .properties
        .iter()
        .find(|(k, _i)| k == &Cow::Borrowed("file"));
    if let Some((_key, file)) = file_prop {
        if let Some(link) = parse_file_link(file) {
            return Some(Page {
                headline,
                page: PageEnum::OrgFile { path: link.into() },
                path: format!("{path}/{}", info.slug),
                info,
                order,
            });
        }
    }

    Some(Page {
        headline,
        page: PageEnum::Post,
        path: format!("{path}/{}", info.slug),

        info,
        order,
    })
}

#[derive(Debug)]
pub struct PageInfo<'a> {
    pub properties: HashMap<String, String>,

    pub title: String,
    pub slug: String,
    pub description: Option<String>,
    pub closed_at: Option<Datetime<'a>>,
}

impl<'a> PageInfo<'a> {
    fn new(title: &'a Title) -> Self {
        let properties: HashMap<String, String> = title
            .properties
            .iter()
            .map(|(a, b)| (a.to_string(), b.to_string()))
            .collect();

        let title_string = properties
            .get("title")
            .cloned()
            .unwrap_or_else(|| title.raw.to_string());
        let slug = slugify(
            properties
                .get("slug")
                .cloned()
                .unwrap_or_else(|| title_string.clone()),
        );
        let description = properties.get("description").cloned();
        let closed_at = title.closed().and_then(|c| {
            if let Timestamp::Inactive { start, .. } = c {
                Some(start.clone())
            } else {
                None
            }
        });

        Self {
            properties,
            title: title_string,
            slug,
            description,
            closed_at,
        }
    }

    pub fn closed_at(&self) -> Option<String> {
        self.closed_at
            .as_ref()
            .map(|d| format!("{}-{:0>2}-{:0>2}", d.year, d.month, d.day))
    }
}

#[cfg(test)]
mod tests {
    use crate::config::TODO_KEYWORDS;

    use super::*;

    #[test]
    fn parse_single() {
        let source = r#"
* index page
this is my content
"#;

        let org = Org::parse(source);
        let page = Page::parse_index(
            &org,
            org.document().first_child(&org).unwrap(),
            &TODO_KEYWORDS,
            "".to_string(),
            0,
            false,
        );

        assert_eq!("index page", page.info.title);
        assert_eq!("index-page", page.info.slug);
        assert_eq!(0, page.order);
        assert_eq!("/index-page", page.path);
        assert_eq!(None, page.info.description);
        assert!(page.info.closed_at.is_none());

        assert!(matches!(page.page, PageEnum::Index { .. }));
        if let PageEnum::Index { children } = page.page {
            assert_eq!(0, children.len());
        }
    }

    #[test]
    fn parse_single_full() {
        let source = r#"
* index
CLOSED: [1234-12-34 Sat 12:34]
:PROPERTIES:
:description: this is a description
:END:
this is my content
"#;

        let org = Org::parse(source);
        let page = Page::parse_index(
            &org,
            org.document().first_child(&org).unwrap(),
            &TODO_KEYWORDS,
            "".to_string(),
            0,
            false,
        );

        assert_eq!("index", page.info.title);
        assert_eq!("index", page.info.slug);
        assert_eq!("/", page.path);
        assert_eq!(0, page.order);
        assert_eq!(
            Some("this is a description"),
            page.info.description.as_deref()
        );
        assert_eq!(Some("1234-12-34"), page.info.closed_at().as_deref());

        assert!(matches!(page.page, PageEnum::Index { .. }));
        if let PageEnum::Index { children } = page.page {
            assert_eq!(0, children.len());
        }
    }

    #[test]
    fn parse_with_children() {
        let source = r#"
* index
index content
** first child
first content
** second child
second content
"#;

        let org = Org::parse(source);
        let page = Page::parse_index(
            &org,
            org.document().first_child(&org).unwrap(),
            &TODO_KEYWORDS,
            "".to_string(),
            0,
            false,
        );

        assert_eq!("index", page.info.slug);
        assert_eq!("/", page.path);
        assert_eq!(0, page.order);

        if let PageEnum::Index { children } = page.page {
            assert_eq!(2, children.len());

            if let Some(page) = children.get("first-child") {
                assert_eq!("first child", page.info.title);
                assert_eq!("first-child", page.info.slug);
            } else {
                panic!("Page is missing");
            }

            if let Some(page) = children.get("second-child") {
                assert_eq!("second child", page.info.title);
                assert_eq!("second-child", page.info.slug);
            } else {
                panic!("Page is missing");
            }
        } else {
            panic!("Page is not an Index");
        }
    }
}

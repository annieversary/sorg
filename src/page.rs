use color_eyre::{eyre::Context, Result};
use orgize::{elements::Title, Headline, Org};
use slugmin::slugify;
use std::{borrow::Cow, collections::HashMap, path::PathBuf};
use tera::Tera;

use crate::{
    context::*,
    helpers::parse_file_link,
    template::{get_template, render_template},
    Config, Keywords,
};

#[derive(Debug)]
pub enum PageEnum<'a> {
    Index { children: HashMap<String, Page<'a>> },
    Post,
    OrgFile { path: PathBuf },
}

pub struct Page<'a> {
    pub headline: Headline,
    org: &'a Org<'a>,

    pub slug: String,
    pub title: String,
    pub path: String,
    pub description: Option<String>,

    pub page: PageEnum<'a>,
}

impl<'a> std::fmt::Debug for Page<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Page")
            // .field("headline", &self.headline)
            .field("slug", &self.slug)
            .field("page", &self.page)
            .finish()
    }
}

impl<'a> Page<'a> {
    pub fn parse_index(
        org: &'a Org<'a>,
        headline: Headline,
        keywords: &Keywords,
        mut path: String,
    ) -> Self {
        let title = headline.title(org);
        let title_string = get_property(title, "title")
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_else(|| title.raw.to_string());

        let slug = get_slug(title, &title_string);
        if slug != "index" {
            path = format!("{path}/{}", slug);
        }

        let description = get_property(title, "description")
            .as_ref()
            .map(ToString::to_string);

        let parent_is_posts = title.tags.contains(&Cow::Borrowed("posts"));

        let children = headline
            .children(org)
            .filter_map(|page| -> Option<Page> {
                let title = page.title(org);
                if title.tags.contains(&Cow::Borrowed("noexport")) {
                    return None;
                }

                let title_string = get_property(title, "title")
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| title.raw.to_string());
                let slug = get_slug(title, &title_string);
                let description = get_property(title, "description")
                    .as_ref()
                    .map(ToString::to_string);

                if title.tags.contains(&Cow::Borrowed("post")) || parent_is_posts {
                    // if there's a keyword, and it's in PROGRESS, we skip it
                    if let Some(kw) = &title.keyword {
                        if keywords.0.contains(&kw.to_string()) {
                            return None;
                        }
                    }

                    // check if it's a linked file
                    let file_prop = title
                        .properties
                        .iter()
                        .find(|(k, _i)| k == &Cow::Borrowed("file"));
                    if let Some((_key, file)) = file_prop {
                        if let Some(link) = parse_file_link(file) {
                            return Some(Page {
                                headline: page,
                                org,
                                page: PageEnum::OrgFile { path: link.into() },
                                path: format!("{path}/{slug}"),
                                slug,
                                title: title_string,
                                description,
                            });
                        }
                    }

                    Some(Page {
                        headline: page,
                        org,
                        page: PageEnum::Post,
                        path: format!("{path}/{slug}"),
                        slug,
                        title: title_string,
                        description,
                    })
                } else {
                    Some(Self::parse_index(org, page, keywords, path.clone()))
                }
            })
            .map(|p| (p.slug.clone(), p))
            .collect();

        if path.is_empty() {
            path = "/".to_string();
        }

        Page {
            headline,
            org,
            page: PageEnum::Index { children },
            slug,
            path,
            title: title_string,
            description,
        }
    }

    pub fn render(&self, tera: &'a Tera, out: &str, config: &Config) -> Result<()> {
        let title = self.headline.title(self.org);
        let properties = title.properties.clone().into_hash_map();

        let out_path = if self.slug == "index" {
            out.to_string()
        } else {
            format!("{out}/{}", self.slug)
        };

        let context = match &self.page {
            PageEnum::Index { children } => {
                get_index_context(&self.headline, self.org, children, config)
            }
            PageEnum::Post => get_post_context(&self.headline, self.org, config),
            PageEnum::OrgFile { path } => {
                get_org_file_context(&self.headline, self.org, path, config)?
            }
        };

        let template = get_template(
            tera,
            &properties,
            &self.path,
            matches!(self.page, PageEnum::Index { .. }),
        );

        if config.verbose {
            println!("writing {out_path}");
        }

        render_template(tera, &template, &context, &out_path)
            .with_context(|| format!("rendering {}", title.raw))?;

        if let PageEnum::Index { children } = &self.page {
            for child in children.values() {
                child.render(tera, &out_path, config)?;
            }
        }
        Ok(())
    }
}

fn get_slug(title: &Title, title_string: &str) -> String {
    if let Some(prop) = get_property(title, "slug") {
        prop.to_string()
    } else {
        slugify(title_string)
    }
}

pub fn get_property<'a>(title: &'_ Title<'a>, prop: &str) -> Option<Cow<'a, str>> {
    title
        .properties
        .iter()
        .find(|(n, _)| n.to_lowercase() == prop)
        .map(|a| a.1.clone())
}

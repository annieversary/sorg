use color_eyre::Result;
use orgize::{elements::Title, Headline, Org};
use slugmin::slugify;
use std::{borrow::Cow, collections::HashMap, path::PathBuf};
use tera::Tera;

use crate::{
    context::*,
    helpers::parse_file_link,
    template::{get_template, render_template},
    Keywords,
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
    pub fn parse_index(org: &'a Org<'a>, headline: Headline, keywords: &Keywords) -> Self {
        let children = headline
            .children(org)
            .filter_map(|page| -> Option<Page> {
                let title = page.title(org);
                if title.tags.contains(&Cow::Borrowed("noexport")) {
                    return None;
                }

                let slug = get_slug(title);

                if title.tags.contains(&Cow::Borrowed("post")) {
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
                                slug,
                                title: title.raw.to_string(),
                            });
                        }
                    }

                    Some(Page {
                        headline: page,
                        org,
                        page: PageEnum::Post,
                        slug,
                        title: title.raw.to_string(),
                    })
                } else {
                    Some(Self::parse_index(org, page, keywords))
                }
            })
            .map(|p| (p.slug.clone(), p))
            .collect();

        Page {
            headline,
            org,
            page: PageEnum::Index { children },
            slug: get_slug(headline.title(org)),
            title: headline.title(org).raw.to_string(),
        }
    }

    pub fn render(&self, tera: &'a Tera, out: &str) -> Result<()> {
        let title = self.headline.title(self.org);
        let properties = title.properties.clone().into_hash_map();
        let name = &title.raw;

        let out_path = get_out(title, out);
        let context = match &self.page {
            PageEnum::Index { children } => get_index_context(&self.headline, self.org, children),
            PageEnum::Post => get_post_context(&self.headline, self.org),
            PageEnum::OrgFile { path } => get_org_file_context(&self.headline, self.org, path)?,
        };
        let template = get_template(
            tera,
            &properties,
            name,
            matches!(self.page, PageEnum::Index { .. }),
        );

        println!("writing {out_path}");
        render_template(tera, &template, &context, &out_path)?;

        if let PageEnum::Index { children } = &self.page {
            for child in children.values() {
                child.render(tera, &out_path)?;
            }
        }
        Ok(())
    }
}

fn get_slug(title: &Title) -> String {
    let properties = title.properties.clone().into_hash_map();
    let name = &title.raw;

    if let Some(prop) = properties.get("slug") {
        prop.to_string()
    } else {
        slugify(name)
    }
}

fn get_out(title: &Title, out: &str) -> String {
    let slug = get_slug(title);
    if slug == "index" {
        out.to_string()
    } else {
        format!("{out}/{slug}")
    }
}

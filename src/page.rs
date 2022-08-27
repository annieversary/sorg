use color_eyre::Result;
use orgize::{Headline, Org};
use slugmin::slugify;
use std::{borrow::Cow, collections::HashMap, path::PathBuf};
use tera::Tera;

use crate::{
    context::*,
    helpers::parse_file_link,
    template::{get_template, render_template},
    Keywords,
};

pub enum PageEnum<'a> {
    Index { children: Vec<Page<'a>> },
    Post,
    OrgFile { path: PathBuf },
}

pub struct Page<'a> {
    pub headline: Headline,
    org: &'a Org<'a>,
    page: PageEnum<'a>,
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

                if title.tags.contains(&Cow::Borrowed("post")) {
                    // if there's a keyword, and it's in PROGRESS, we skip it
                    if let Some(kw) = &title.keyword {
                        // TODO we shouldn't need to call to_string here
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
                            });
                        }
                    }

                    Some(Page {
                        headline: page,
                        org,
                        page: PageEnum::Post,
                    })
                } else {
                    Some(Self::parse_index(org, page, keywords))
                }
            })
            .collect();

        Page {
            headline,
            org,
            page: PageEnum::Index { children },
        }
    }

    pub fn render(&self, tera: &'a Tera, out: &str) -> Result<()> {
        let title = self.headline.title(self.org);
        let properties = title.properties.clone().into_hash_map();
        let name = &title.raw;

        let template = get_template(tera, &properties, name);
        let out_path = get_out(&properties, name, out);
        let context = match &self.page {
            PageEnum::Index { children } => get_index_context(&self.headline, self.org, children),
            PageEnum::Post => get_post_context(&self.headline, self.org),
            // TODO open the file and process it individually
            PageEnum::OrgFile { path } => get_post_context(&self.headline, self.org),
        };

        println!("writing {out_path}");
        render_template(tera, &template, &context, &out_path)?;

        if let PageEnum::Index { children } = &self.page {
            for child in children {
                child.render(tera, &out_path)?;
            }
        }
        Ok(())
    }
}

fn get_out<'a>(properties: &HashMap<Cow<'a, str>, Cow<'a, str>>, name: &str, out: &str) -> String {
    let f = if let Some(prop) = properties.get("out") {
        prop
    } else {
        name
    };
    if f == "index" {
        out.to_string()
    } else {
        let f = slugify(f);
        format!("{out}/{f}")
    }
}

use std::{collections::HashMap, fs::File, io::Read, path::Path, rc::Rc};

use color_eyre::{eyre::WrapErr, Result};
use orgize::{Headline, Org};
use serde_derive::Serialize;
use tera::Context;

use crate::{
    count_words::*,
    footnotes::*,
    macros::Macro,
    page::{Page, PageEnum},
    render::*,
    Config,
};

impl Page<'_> {
    pub fn page_context(
        &self,
        org: &Org<'_>,
        macros: Rc<HashMap<String, Macro>>,
        config: &Config,
    ) -> Result<Context> {
        let mut context = match &self.page {
            PageEnum::Index { children } => {
                get_index_context(&self.headline, org, children, macros, config)
            }
            PageEnum::Post => get_post_context(&self.headline, org, macros, config),
            PageEnum::OrgFile { path } => {
                get_org_file_context(&self.headline, org, path, macros, config)?
            }
        };

        context.insert("asset_v", &rand::random::<u16>());

        context.insert("title", &self.info.title);
        context.insert("date", &self.info.closed_at());

        context.insert("base_title", &config.title);
        context.insert("base_url", &config.url);
        context.insert("base_description", &config.description);

        for (k, v) in &self.info.properties {
            context.insert(k.clone(), &v);
        }

        Ok(context)
    }
}

#[derive(Serialize, Debug)]
pub struct PageLink<'a> {
    title: &'a str,
    slug: &'a str,
    description: Option<&'a str>,
    order: usize,
    closed_at: Option<String>,
}

fn get_index_context(
    headline: &Headline,
    org: &Org<'_>,
    children: &HashMap<String, Page>,
    macros: Rc<HashMap<String, Macro>>,
    config: &Config,
) -> Context {
    let mut pages = children
        .iter()
        .map(|(slug, page)| PageLink {
            slug,
            title: &page.info.title,
            description: page.info.description.as_deref(),
            order: page.order,
            closed_at: page.info.closed_at(),
        })
        .collect::<Vec<_>>();
    pages.sort_unstable_by(|a, b| a.order.cmp(&b.order));

    let html = write_html(
        headline,
        org,
        IndexHtmlHandler {
            level: headline.level(),
            handler: CommonHtmlHandler {
                handler: html_handler(config.syntax_highlighting_theme.clone()),
                config: config.clone(),
                attributes: Default::default(),
                footnote_id: 0,
                macros,
            },
            in_headline: false,
            in_page_title: false,
        },
    );

    let mut context = Context::new();
    context.insert("content", &html);
    context.insert("pages", &pages);

    let word_count = count_words_index(headline, org);
    context.insert("word_count", &word_count);
    context.insert("reading_time", &(word_count / 180).max(1));

    context
}

/// generates the context for a blog post
///
/// renders the contents and gets the sections and stuff
fn get_post_context(
    headline: &Headline,
    org: &Org<'_>,
    macros: Rc<HashMap<String, Macro>>,
    config: &Config,
) -> Context {
    let sections = headline
        .children(org)
        .map(|h| h.title(org).raw.clone())
        .collect::<Vec<_>>();

    let mut context = Context::new();

    let handler = PostHtmlHandler {
        level: headline.level(),
        handler: CommonHtmlHandler {
            handler: html_handler(config.syntax_highlighting_theme.clone()),
            config: config.clone(),
            attributes: Default::default(),
            footnote_id: 0,
            macros,
        },
        in_page_title: false,
    };
    let html = write_html(headline, org, handler);

    context.insert("content", &html);
    context.insert("sections", &sections);

    let word_count = count_words_post(headline, org);
    context.insert("word_count", &word_count);
    context.insert("reading_time", &(word_count / 180).max(1));

    let footnotes = get_footnotes(org, headline);
    context.insert("footnotes", &footnotes);

    context
}

fn get_org_file_context(
    headline: &Headline,
    org: &Org<'_>,
    file: &Path,
    macros: Rc<HashMap<String, Macro>>,
    config: &Config,
) -> Result<Context> {
    let sections = headline
        .children(org)
        .map(|h| h.title(org).raw.clone())
        .collect::<Vec<_>>();

    let title = headline.title(org);

    let mut context = Context::new();

    let mut f = File::open(file).wrap_err_with(|| {
        format!(
            "headline '{}' tried to read file '{}', which doesnt exist",
            title.raw,
            file.as_os_str().to_string_lossy()
        )
    })?;
    let mut src = String::new();
    f.read_to_string(&mut src)?;

    let new_org = Org::parse(&src);
    let doc = new_org.document();
    let first = doc.first_child(&new_org).unwrap();

    let html = write_html(
        &first,
        &new_org,
        PostHtmlHandler {
            level: first.level(),
            handler: CommonHtmlHandler {
                handler: html_handler(config.syntax_highlighting_theme.clone()),
                config: config.clone(),
                attributes: Default::default(),
                footnote_id: 0,
                macros,
            },
            in_page_title: false,
        },
    );

    context.insert("content", &html);
    context.insert("sections", &sections);

    let word_count = count_words_post(&first, org);
    context.insert("word_count", &word_count);
    context.insert("reading_time", &(word_count / 180).max(1));

    let footnotes = get_footnotes(org, headline);
    context.insert("footnotes", &footnotes);

    Ok(context)
}

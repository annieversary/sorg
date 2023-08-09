use std::{
    collections::{BTreeMap, HashMap},
    fs::File,
    io::Read,
    marker::PhantomData,
    path::Path,
    sync::OnceLock,
};

use color_eyre::{eyre::WrapErr, Result};
use orgize::{
    elements::{FnDef, FnRef, Link},
    export::{DefaultHtmlHandler, SyntectHtmlHandler},
    indextree::NodeEdge,
    syntect::{
        highlighting::{Theme, ThemeSet},
        html::IncludeBackground,
        parsing::SyntaxSet,
    },
    Element, Event, Headline, Org,
};
use serde_derive::Serialize;
use tera::Context;

use crate::{page::Page, render::*, Config};

static SYNTECT: OnceLock<(SyntaxSet, BTreeMap<String, Theme>)> = OnceLock::new();

fn html_handler() -> SyntectHtmlHandler<std::io::Error, DefaultHtmlHandler> {
    let (syntax_set, themes) = SYNTECT.get_or_init(|| {
        (
            SyntaxSet::load_defaults_newlines(),
            ThemeSet::load_defaults().themes,
        )
    });

    SyntectHtmlHandler {
        syntax_set: syntax_set.clone(),
        theme_set: ThemeSet {
            themes: themes.clone(),
        },
        theme: String::from("InspiredGitHub"),
        inner: DefaultHtmlHandler,
        background: IncludeBackground::No,
        error_type: PhantomData,
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

pub fn get_index_context(
    headline: &Headline,
    org: &Org<'_>,
    children: &HashMap<String, Page>,
    config: &Config,
) -> Context {
    let mut pages = children
        .iter()
        .map(|(slug, page)| PageLink {
            slug,
            title: &page.title,
            description: page.description.as_deref(),
            order: page.order,
            closed_at: page
                .closed_at
                .as_ref()
                .map(|d| format!("{}-{:0>2}-{:0>2}", d.year, d.month, d.day)),
        })
        .collect::<Vec<_>>();
    pages.sort_unstable_by(|a, b| a.order.cmp(&b.order));

    let title = headline.title(org);

    let html = write_html(
        headline,
        org,
        IndexHtmlHandler {
            level: headline.level(),
            handler: CommonHtmlHandler {
                handler: html_handler(),
                config: config.clone(),
                attributes: Default::default(),
                footnote_id: 0,
            },
            in_headline: false,
            in_page_title: false,
        },
    );

    let mut context = Context::new();
    context.insert("title", &title.raw);
    context.insert("content", &html);
    context.insert("pages", &pages);
    let word_count = count_words_index(headline, org);
    context.insert("word_count", &word_count);
    context.insert("reading_time", &(word_count / 180).max(1));

    for (k, v) in headline.title(org).properties.iter() {
        context.insert(k.clone(), v);
    }

    context
}

#[derive(Serialize)]
struct Footnote {
    label: String,
    definition: String,
}

fn get_footnotes(org: &Org<'_>, headline: &Headline) -> Vec<Footnote> {
    let it = headline
        .headline_node()
        .traverse(org.arena())
        .map(move |edge| match edge {
            NodeEdge::Start(node) => Event::Start(&org[node]),
            NodeEdge::End(node) => Event::End(&org[node]),
        });

    let mut footnotes = Vec::new();

    let mut footnote_id = 0;
    let mut in_footnote = None;

    for event in it {
        // println!("sub: {in_subheadline}, head: {in_headline}, title: {in_page_title}");
        match event {
            Event::Start(element) => match element {
                Element::FnDef(FnDef { label, .. }) => {
                    in_footnote = Some((label.to_string(), "".to_string()));
                }
                Element::FnRef(FnRef {
                    label,
                    definition: Some(def),
                }) => {
                    let label = if label.is_empty() {
                        footnote_id += 1;
                        footnote_id.to_string()
                    } else {
                        label.to_string()
                    };
                    footnotes.push(Footnote {
                        label,
                        definition: def.to_string(),
                    });
                }
                Element::Text { value } if in_footnote.is_some() => {
                    if let Some((_, def)) = &mut in_footnote {
                        def.push_str(value);
                    }
                }
                _ => {}
            },
            Event::End(element) => {
                if let Element::FnDef(_) = element {
                    if let Some((label, definition)) = in_footnote.take() {
                        footnotes.push(Footnote { label, definition });
                    }
                }
            }
        }
    }

    footnotes
}

/// generates the context for a blog post
///
/// renders the contents and gets the sections and stuff
pub fn get_post_context(
    headline: &Headline,
    org: &Org<'_>,
    config: &Config,
    page: &Page,
) -> Context {
    let sections = headline
        .children(org)
        .map(|h| h.title(org).raw.clone())
        .collect::<Vec<_>>();

    let title = headline.title(org);

    let mut context = Context::new();
    context.insert("title", &title.raw);
    context.insert(
        "date",
        &page
            .closed_at
            .as_ref()
            .map(|d| format!("{}-{:0>2}-{:0>2}", d.year, d.month, d.day)),
    );
    let word_count = count_words_post(headline, org);
    context.insert("word_count", &word_count);
    context.insert("reading_time", &(word_count / 180).max(1));

    let footnotes = get_footnotes(org, headline);
    context.insert("footnotes", &footnotes);

    let handler = PostHtmlHandler {
        level: headline.level(),
        handler: CommonHtmlHandler {
            handler: html_handler(),
            config: config.clone(),
            attributes: Default::default(),
            footnote_id: 0,
        },
        in_page_title: false,
    };
    // handler.handler.theme = "Solarized (light)".into();
    let html = write_html(headline, org, handler);

    context.insert("content", &html);
    context.insert("sections", &sections);

    for (k, v) in headline.title(org).properties.iter() {
        context.insert(k.clone(), v);
    }

    context
}
pub fn get_org_file_context(
    headline: &Headline,
    org: &Org<'_>,
    file: &Path,
    config: &Config,
) -> Result<Context> {
    let sections = headline
        .children(org)
        .map(|h| h.title(org).raw.clone())
        .collect::<Vec<_>>();

    let title = headline.title(org);

    let mut context = Context::new();
    context.insert("title", &title.raw);

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
                handler: html_handler(),
                config: config.clone(),
                attributes: Default::default(),
                footnote_id: 0,
            },
            in_page_title: false,
        },
    );

    context.insert("content", &html);
    context.insert("sections", &sections);
    let word_count = count_words_post(&first, org);
    context.insert("word_count", &word_count);
    context.insert("reading_time", &(word_count / 180).max(1));

    for (k, v) in headline.title(org).properties.iter() {
        context.insert(k.clone(), v);
    }

    Ok(context)
}

fn count_words_index(headline: &Headline, org: &Org<'_>) -> usize {
    // dont count children headlines, just the actual text on this page
    let it = headline
        .headline_node()
        .traverse(org.arena())
        .map(move |edge| match edge {
            NodeEdge::Start(node) => Event::Start(&org[node]),
            NodeEdge::End(node) => Event::End(&org[node]),
        });

    let mut v = 0;
    let mut in_subheadline = false;
    let mut in_headline = false;
    let mut in_page_title = false;

    for event in it {
        // println!("sub: {in_subheadline}, head: {in_headline}, title: {in_page_title}");
        match event {
            Event::Start(element) if !in_headline => match element {
                Element::Headline { level } if *level > headline.level() + 1 => {
                    in_headline = true;
                }
                Element::Headline { level } if *level > headline.level() => {
                    in_subheadline = true;
                }
                Element::Title(_) => {
                    in_page_title = true;
                }
                _ if !in_subheadline || in_page_title => {
                    // count this element
                    v += match element {
                        Element::Text { value } => words_count::count(value).words,
                        Element::Link(Link {
                            desc: Some(value), ..
                        }) => words_count::count(value).words,
                        _ => 0,
                    };
                }
                // while we are in a title, we'll land here, cause we dont want to show the child posts
                _ => {}
            },
            Event::End(element) => match element {
                Element::Headline { level } if *level > headline.level() + 1 => {
                    in_headline = false;
                }
                Element::Headline { level } if *level > headline.level() => {
                    in_subheadline = false;
                }
                Element::Title(_) => {
                    in_page_title = false;
                }
                _ => {}
            },
            _ => {}
        }
    }

    v
}
fn count_words_post(headline: &Headline, org: &Org<'_>) -> usize {
    headline
        .headline_node()
        .traverse(org.arena())
        .flat_map(move |edge| match edge {
            NodeEdge::Start(node) => Some(&org[node]),
            NodeEdge::End(_) => None,
        })
        .map(|el| match el {
            Element::Text { value } => words_count::count(value).words,
            Element::Link(Link {
                desc: Some(value), ..
            }) => words_count::count(value).words,
            _ => 0,
        })
        .sum()
}

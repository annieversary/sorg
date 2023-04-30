use std::{
    borrow::Cow,
    fs::File,
    io::{Read, Write},
    path::Path,
};

use color_eyre::{eyre::WrapErr, Report, Result};
use orgize::{
    elements::Timestamp,
    export::{DefaultHtmlHandler, HtmlHandler},
    indextree::NodeEdge,
    Element, Event, Headline, Org,
};
use serde_derive::Serialize;
use slugmin::slugify;
use tera::Context;

use crate::page::Page;

#[derive(Serialize)]
struct PageLink<'a> {
    title: &'a str,
    slug: Cow<'a, str>,
    description: Option<Cow<'a, str>>,
}

pub fn get_index_context(headline: &Headline, org: &Org<'_>, children: &[Page]) -> Context {
    let pages = children
        .iter()
        .map(|h| {
            let t = h.headline.title(org);
            let title = t.raw.as_ref();
            let slug = t
                .properties
                .iter()
                .find(|(n, _)| n == "slug")
                .map(|a| a.1.clone())
                .unwrap_or_else(|| Cow::Owned(slugify(title)));
            let description = t
                .properties
                .iter()
                .find(|(n, _)| n == "description")
                .map(|a| a.1.clone());

            PageLink {
                slug,
                title,
                description,
            }
        })
        .collect::<Vec<_>>();

    let title = headline.title(org);

    let html = write_html(
        headline,
        org,
        IndexHtmlHandler {
            level: headline.level(),
            ..Default::default()
        },
    );

    let mut context = Context::new();
    context.insert("title", &title.raw);
    context.insert("content", &html);
    context.insert("pages", &pages);

    for (k, v) in headline.title(org).properties.iter() {
        context.insert(k.clone(), v);
    }

    context
}

/// generates the context for a blog post
///
/// renders the contents and gets the sections and stuff
pub fn get_post_context(headline: &Headline, org: &Org<'_>) -> Context {
    let sections = headline
        .children(org)
        .map(|h| h.title(org).raw.clone())
        .collect::<Vec<_>>();

    let title = headline.title(org);

    let closed = title.closed().and_then(|c| {
        if let Timestamp::Inactive { start, .. } = c {
            Some(start)
        } else {
            None
        }
    });

    let mut context = Context::new();
    context.insert("title", &title.raw);
    context.insert("date", &closed);

    let handler = PostHtmlHandler {
        level: headline.level(),
        ..Default::default()
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
pub fn get_org_file_context<'a>(
    headline: &Headline,
    org: &Org<'a>,
    file: &Path,
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
            ..Default::default()
        },
    );

    context.insert("content", &html);
    context.insert("sections", &sections);

    for (k, v) in headline.title(org).properties.iter() {
        context.insert(k.clone(), v);
    }

    Ok(context)
}

/// renders html for a post
pub fn write_html(
    headline: &Headline,
    org: &Org<'_>,
    mut handler: impl HtmlHandler<Report>,
) -> String {
    let it = headline
        .headline_node()
        .traverse(org.arena())
        .map(move |edge| match edge {
            NodeEdge::Start(node) => Event::Start(&org[node]),
            NodeEdge::End(node) => Event::End(&org[node]),
        });

    let mut w = Vec::new();

    for event in it {
        match event {
            Event::Start(element) => handler
                .start(&mut w, element)
                .expect("failed to write to html"),
            Event::End(element) => handler
                .end(&mut w, element)
                .expect("failed to write to html"),
        }
    }

    String::from_utf8(w).expect("org file should contain valid utf8")
}

#[derive(Default)]
struct IndexHtmlHandler {
    handler: DefaultHtmlHandler,
    // handler: SyntectHtmlHandler<std::io::Error, DefaultHtmlHandler>,
    level: usize,
    in_headline: bool,
    in_page_title: bool,
}

impl HtmlHandler<Report> for IndexHtmlHandler {
    fn start<W: Write>(&mut self, w: W, element: &Element) -> Result<()> {
        match element {
            Element::Headline { level } if *level > self.level => {
                self.in_headline = true;
            }
            Element::Title(_) => {
                self.in_page_title = true;
            }
            _ if !self.in_page_title && !self.in_headline => {
                // fallthrough to default handler
                self.handler.start(w, element)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn end<W: Write>(&mut self, w: W, element: &Element) -> Result<()> {
        match element {
            Element::Headline { level } if *level > self.level => {
                self.in_headline = false;
            }
            Element::Title(_) => {
                self.in_page_title = false;
            }
            _ if !self.in_page_title && !self.in_headline => {
                // fallthrough to default handler
                self.handler.end(w, element)?;
            }
            _ => {}
        }
        Ok(())
    }
}

#[derive(Default)]
struct PostHtmlHandler {
    handler: DefaultHtmlHandler,
    // handler: SyntectHtmlHandler<std::io::Error, DefaultHtmlHandler>,
    level: usize,
    in_page_title: bool,
}

impl HtmlHandler<Report> for PostHtmlHandler {
    fn start<W: Write>(&mut self, mut w: W, element: &Element) -> Result<()> {
        match element {
            Element::Title(title) if title.level == self.level => {
                self.in_page_title = true;
            }
            Element::Title(title) => {
                write!(
                    w,
                    "<h{0}><a id=\"{1}\" href=\"#{1}\">",
                    title.level - self.level,
                    slugify(&title.raw),
                )?;
            }
            _ if !self.in_page_title => {
                // fallthrough to default handler
                self.handler.start(w, element)?;
            }
            _ => {}
        }
        Ok(())
    }

    fn end<W: Write>(&mut self, mut w: W, element: &Element) -> Result<()> {
        match element {
            Element::Title(title) if title.level == self.level => {
                self.in_page_title = false;
            }
            Element::Title(title) => {
                write!(w, "</a></h{}>", title.level - self.level)?;
            }
            _ if !self.in_page_title => {
                // fallthrough to default handler
                self.handler.end(w, element)?;
            }
            _ => {}
        }
        Ok(())
    }
}

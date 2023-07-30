use std::{
    borrow::Cow,
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    path::Path,
};

use color_eyre::{eyre::WrapErr, Report, Result};
use orgize::{
    elements::Timestamp,
    export::{DefaultHtmlHandler, HtmlEscape, HtmlHandler},
    indextree::NodeEdge,
    Element, Event, Headline, Org,
};
use serde_derive::Serialize;
use slugmin::slugify;
use tera::Context;

use crate::{page::Page, Config};

#[derive(Serialize, Debug)]
struct PageLink<'a> {
    title: &'a str,
    slug: String,
    description: Option<Cow<'a, str>>,
}

pub fn get_index_context(
    headline: &Headline,
    org: &Org<'_>,
    children: &HashMap<String, Page>,
    config: &Config,
) -> Context {
    let pages = children
        .iter()
        .map(|(slug, h)| {
            let t = h.headline.title(org);
            let title = t.raw.as_ref();
            let description = t
                .properties
                .iter()
                .find(|(n, _)| n == "description")
                .map(|a| a.1.clone());

            PageLink {
                slug: slug.clone(),
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
            handler: CommonHtmlHandler {
                handler: DefaultHtmlHandler,
                config: config.clone(),
                ..Default::default()
            },
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
pub fn get_post_context(headline: &Headline, org: &Org<'_>, config: &Config) -> Context {
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
        handler: CommonHtmlHandler {
            handler: DefaultHtmlHandler,
            config: config.clone(),
            ..Default::default()
        },
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
                handler: DefaultHtmlHandler,
                config: config.clone(),
                ..Default::default()
            },
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
    handler: CommonHtmlHandler,
    // handler: SyntectHtmlHandler<std::io::Error, DefaultHtmlHandler>,
    level: usize,
    in_headline: bool,
    in_page_title: bool,
}

impl HtmlHandler<Report> for IndexHtmlHandler {
    fn start<W: Write>(&mut self, w: W, element: &Element) -> Result<()> {
        // skips titles at same level as the level we are on
        // unsure what this actually does
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
    handler: CommonHtmlHandler,
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
                    "<h{0} {2}><a id=\"{1}\" href=\"#{1}\">",
                    title.level - self.level + 1,
                    slugify(&title.raw),
                    self.handler.render_attributes(""),
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
                write!(w, "</a></h{}>", title.level - self.level + 1)?;
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

#[derive(Default)]
struct CommonHtmlHandler {
    handler: DefaultHtmlHandler,
    config: Config,

    attributes: HashMap<String, String>,
}

impl CommonHtmlHandler {
    fn render_attributes(&mut self, class: &str) -> String {
        if !class.is_empty() {
            self.attributes
                .entry("class".to_string())
                .and_modify(|s| {
                    s.push(' ');
                    s.push_str(class);
                })
                .or_insert_with(|| class.to_string());
        }

        self.attributes
            .iter()
            .map(|(k, v)| format!(" {k}=\"{v}\" "))
            .collect()
    }
}

impl HtmlHandler<Report> for CommonHtmlHandler {
    fn start<W: Write>(&mut self, mut w: W, element: &Element) -> Result<()> {
        match element {
            Element::Paragraph { .. } => write!(w, "<p {}>", self.render_attributes(""))?,
            Element::QuoteBlock(_) => write!(w, "<blockquote {}>", self.render_attributes(""))?,
            Element::CenterBlock(_) => write!(w, "<div {}>", self.render_attributes("center"))?,
            Element::VerseBlock(_) => write!(w, "<p {}>", self.render_attributes("verse"))?,
            Element::Bold => write!(w, "<b {}>", self.render_attributes(""))?,
            Element::List(list) => {
                if list.ordered {
                    write!(w, "<ol {}", self.render_attributes("v"))?;
                } else {
                    write!(w, "<ul {}>", self.render_attributes(""))?;
                }
            }
            Element::Italic => write!(w, "<i {}>", self.render_attributes(""))?,
            Element::ListItem(_) => write!(w, "<li {}>", self.render_attributes(""))?,
            Element::Section => write!(w, "<section {}>", self.render_attributes(""))?,
            Element::Strike => write!(w, "<s {}>", self.render_attributes(""))?,
            Element::Underline => write!(w, "<u {}>", self.render_attributes(""))?,
            Element::Document { .. } => write!(w, "<main {}>", self.render_attributes(""))?,
            Element::Title(title) => {
                write!(
                    w,
                    "<h{} {}>",
                    if title.level <= 6 { title.level } else { 6 },
                    self.render_attributes("")
                )?;
            }

            Element::Link(link) => {
                let path = link.path.trim_start_matches("file:");
                let path =
                    path.trim_start_matches(format!("./{}", self.config.static_path).as_str());
                let attrs = self.render_attributes("");

                if path.ends_with(".jpg")
                    || path.ends_with(".jpeg")
                    || path.ends_with(".png")
                    || path.ends_with(".gif")
                    || path.ends_with(".webp")
                {
                    write!(w, "<img src=\"{}\" {attrs} />", HtmlEscape(path),)?
                } else {
                    write!(
                        w,
                        "<a href=\"{}\" {attrs}>{}</a>",
                        HtmlEscape(&path),
                        HtmlEscape(link.desc.as_ref().unwrap_or(&Cow::Borrowed(path))),
                    )?
                }
            }
            Element::Keyword(keyword) => {
                if keyword.key.to_lowercase() == "caption" {
                    self.attributes
                        .insert("alt".to_string(), keyword.value.to_string());
                    self.attributes
                        .insert("title".to_string(), keyword.value.to_string());
                }
                if keyword.key.to_lowercase() == "attr_html" {
                    // TODO make this accept multiple things
                    let v = keyword.value.trim_start_matches(':');
                    if let Some((k, v)) = v.split_once(' ') {
                        self.attributes.insert(k.to_string(), v.to_string());
                    }
                }
            }

            _ => {
                self.handler.start(w, element)?;
            }
        }
        Ok(())
    }

    fn end<W: Write>(&mut self, w: W, element: &Element) -> Result<()> {
        match element {
            Element::Keyword(_k) => {}
            _ => {
                self.attributes.clear();
                self.handler.end(w, element)?;
            }
        }

        Ok(())
    }
}

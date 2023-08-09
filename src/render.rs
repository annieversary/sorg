use std::{borrow::Cow, collections::HashMap, io::Write};

use color_eyre::{Report, Result};
use orgize::{
    elements::FnRef,
    export::{DefaultHtmlHandler, HtmlEscape, HtmlHandler, SyntectHtmlHandler},
    indextree::NodeEdge,
    Element, Event, Headline, Org,
};
use slugmin::slugify;

use crate::Config;

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
pub struct IndexHtmlHandler {
    pub handler: CommonHtmlHandler,
    pub level: usize,
    pub in_headline: bool,
    pub in_page_title: bool,
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
            // while we are in a title, we'll land here, cause we dont want to show the child posts
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
pub struct PostHtmlHandler {
    pub handler: CommonHtmlHandler,
    pub level: usize,
    pub in_page_title: bool,
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
pub struct CommonHtmlHandler {
    pub handler: SyntectHtmlHandler<std::io::Error, DefaultHtmlHandler>,
    pub config: Config,

    pub attributes: HashMap<String, String>,
    pub footnote_id: usize,
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
            Element::FnRef(FnRef { label, .. }) => {
                let label = if label.is_empty() {
                    self.footnote_id += 1;
                    Cow::Owned(format!("{}", self.footnote_id))
                } else {
                    label.clone()
                };

                write!(
                    w,
                    r##"<sup id="fnref-{label}"><a href="#fn-{label}" class="footnote-ref">{label}</a></sup>"##
                )?;
            }
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
                    write!(
                        w,
                        "<figure class=\"image\"><img src=\"{}\" {attrs} />",
                        HtmlEscape(path),
                    )?;
                    if let Some(caption) = self.attributes.get("alt") {
                        write!(w, "<figcaption>{}</figcaption>", HtmlEscape(caption))?;
                    }
                    write!(w, "</figure>")?;
                } else {
                    write!(
                        w,
                        "<a href=\"{}\" {attrs}>{}</a>",
                        HtmlEscape(&path),
                        HtmlEscape(link.desc.as_ref().unwrap_or(&Cow::Borrowed(path))),
                    )?;
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

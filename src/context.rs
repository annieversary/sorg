use std::{
    fs::File,
    io::{Read, Write},
};

use color_eyre::{Report, Result};
use orgize::{
    export::{DefaultHtmlHandler, HtmlHandler},
    indextree::NodeEdge,
    Element, Event, Headline, Org,
};
use slugmin::slugify;
use tera::Context;

use crate::page::Page;

pub fn get_index_context<'a>(headline: &Headline, org: &Org<'a>, children: &[Page]) -> Context {
    let pages = children
        .iter()
        .map(|h| h.headline.title(org).raw.to_owned())
        .map(|h| (slugify(&h), h))
        .collect::<Vec<_>>();

    let title = headline.title(org);

    let mut context = Context::new();
    context.insert("title", &title.raw);
    context.insert("content", &"heyyy");
    context.insert("pages", &pages);

    for (k, v) in headline.title(org).properties.iter() {
        context.insert(k.clone(), v);
    }

    context
}
pub fn get_post_context<'a>(headline: &Headline, org: &Org<'a>) -> Context {
    let sections = headline
        .children(org)
        .map(|h| h.title(org).raw.to_owned())
        .collect::<Vec<_>>();

    let title = headline.title(org);

    let mut context = Context::new();
    context.insert("title", &title.raw);

    // TODO find a way to skip the headline, we just want the content

    let html = write_html(headline, org);

    context.insert("content", &html);
    context.insert("sections", &sections);

    for (k, v) in headline.title(org).properties.iter() {
        context.insert(k.clone(), v);
    }

    context
}
pub fn get_org_file_context<'a>(headline: &Headline, org: &Org<'a>, file: &str) -> Result<Context> {
    let sections = headline
        .children(org)
        .map(|h| h.title(org).raw.to_owned())
        .collect::<Vec<_>>();

    let title = headline.title(org);

    let mut context = Context::new();
    context.insert("title", &title.raw);

    // TODO find a way to skip the headline, we just want the content

    let mut f = File::open(file)?;
    let mut src = String::new();
    f.read_to_string(&mut src)?;
    let mut w = Vec::new();
    Org::parse(&src).write_html(&mut w)?;
    let html = String::from_utf8(w)?;

    context.insert("content", &html);
    context.insert("sections", &sections);

    for (k, v) in headline.title(org).properties.iter() {
        context.insert(k.clone(), v);
    }

    Ok(context)
}

pub fn write_html<'a>(headline: &Headline, org: &Org<'a>) -> String {
    let level = headline.level();

    let it = headline
        .headline_node()
        .traverse(org.arena())
        .map(move |edge| match edge {
            NodeEdge::Start(node) => Event::Start(&org[node]),
            NodeEdge::End(node) => Event::End(&org[node]),
        });

    let mut w = Vec::new();
    let mut handler = MyHtmlHandler {
        handler: DefaultHtmlHandler::default(),
        level,
    };

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
struct MyHtmlHandler {
    handler: DefaultHtmlHandler,
    level: usize,
}

impl HtmlHandler<Report> for MyHtmlHandler {
    fn start<W: Write>(&mut self, mut w: W, element: &Element) -> Result<()> {
        if let Element::Title(title) = element {
            write!(
                w,
                "<h{0}><a id=\"{1}\" href=\"#{1}\">",
                title.level - self.level,
                slugify(&title.raw),
            )?;
        } else {
            // fallthrough to default handler
            self.handler.start(w, element)?;
        }
        Ok(())
    }

    fn end<W: Write>(&mut self, mut w: W, element: &Element) -> Result<()> {
        if let Element::Title(title) = element {
            write!(w, "</a></h{}>", title.level - self.level)?;
        } else {
            self.handler.end(w, element)?;
        }
        Ok(())
    }
}

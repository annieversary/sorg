use std::{
    borrow::Cow,
    collections::{BTreeMap, HashMap},
    fmt::Write as FmtWrite,
    io::{Error, Write},
    marker::PhantomData,
    rc::Rc,
    sync::OnceLock,
};

use color_eyre::{eyre::Context as EyreContext, Report, Result};
use orgize::{
    elements::FnRef,
    export::{DefaultHtmlHandler, HtmlEscape, HtmlHandler, SyntectHtmlHandler},
    indextree::NodeEdge,
    syntect::{
        easy::HighlightLines,
        highlighting::{Theme, ThemeSet},
        html::{styled_line_to_highlighted_html, IncludeBackground},
        parsing::SyntaxSet,
    },
    Element, Event, Headline, Org,
};
use slugmin::slugify;
use tera::{Context, Tera};
use vfs::VfsPath;

use crate::{
    macros::Macros,
    page::{Page, PageEnum},
    tera::get_template,
    Config,
};

impl<'a> Page<'a> {
    pub fn render(
        &self,
        tera: &'a Tera,
        out: VfsPath,
        config: &Config,
        org: &Org,
        macros: Rc<Macros>,
        hotreloading: bool,
    ) -> Result<tera::Context> {
        let out_path = if self.info.slug == "index" {
            out
        } else {
            out.join(&self.info.slug)?
        };

        let template = get_template(
            tera,
            self.info.properties.get("template"),
            &self.path,
            matches!(self.page, PageEnum::Index { .. }),
        );

        if config.verbose {
            println!("writing {}", out_path.as_str());
        }

        let context = self.page_context(org, macros.clone(), config)?;

        render_template(tera, &template, &context, out_path.clone(), hotreloading)
            .with_context(|| format!("rendering {}", &self.info.title))?;

        if let PageEnum::Index { children } = &self.page {
            let children = children
                .values()
                .map(|child| -> Result<_> {
                    let context = child.render(
                        tera,
                        out_path.clone(),
                        config,
                        org,
                        macros.clone(),
                        hotreloading,
                    )?;
                    Ok((child, context))
                })
                .collect::<Result<Vec<_>, _>>()?;

            // generate rss feed for this
            let rss_content = crate::rss::generate_rss(children, config, &self.path);
            let mut rss_file = out_path.join("rss.xml")?.create_file()?;
            write!(rss_file, "{}", rss_content)?;
        }
        Ok(context)
    }
}

/// renders the given template to the output path using the provided context
pub fn render_template(
    tera: &Tera,
    template: &str,
    context: &Context,
    out_path: VfsPath,
    hotreloading: bool,
) -> Result<String> {
    let mut content = tera.render(template, context)?;

    if hotreloading {
        content.push_str("<script>(() => { const socket = new WebSocket('ws://localhost:2794', 'sorg'); socket.addEventListener('message', () => {location.reload();}); })();</script>",);
    }

    out_path.create_dir_all()?;

    let mut file = out_path.join("index.html")?.create_file()?;
    file.write_all(content.as_bytes())?;

    Ok(content)
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

static SYNTECT: OnceLock<(SyntaxSet, BTreeMap<String, Theme>)> = OnceLock::new();

pub fn html_handler(
    systax_highlighting_theme: String,
) -> SyntectHtmlHandler<std::io::Error, DefaultHtmlHandler> {
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
        theme: systax_highlighting_theme,
        inner: DefaultHtmlHandler,
        background: IncludeBackground::No,
        error_type: PhantomData,
    }
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

    pub macros: Rc<Macros>,
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
            .fold(String::new(), |mut output, (k, v)| {
                let _ = write!(output, " {k}=\"{v}\" ");
                output
            })
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
            Element::Macros(call) => {
                if let Some(macro_processor) = self.macros.get(call.name.as_ref()) {
                    let args = call.arguments.as_deref().unwrap_or_default();
                    let out = macro_processor.process(args)?;
                    write!(w, "{}", out)?;
                }
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
                let path = link
                    .path
                    .trim_start_matches("file:")
                    .trim_start_matches(format!("./{}", self.config.static_path.as_str()).as_str())
                    .trim_start_matches(format!(".{}", self.config.static_path.as_str()).as_str())
                    .trim_start_matches(format!("/{}", self.config.static_path.as_str()).as_str());
                let mut attrs = self.render_attributes("");

                let lower = path.to_lowercase();

                let base_url = url::Url::parse(&self.config.url)?;
                let url = base_url.join(path)?;

                if lower.ends_with(".jpg")
                    || lower.ends_with(".jpeg")
                    || lower.ends_with(".png")
                    || lower.ends_with(".gif")
                    || lower.ends_with(".webp")
                {
                    write!(
                        w,
                        "<figure class=\"image\"><img src=\"{}\" {attrs} loading=\"lazy\" />",
                        HtmlEscape(url.as_str()),
                    )?;
                    if let Some(caption) = self.attributes.get("alt") {
                        write!(w, "<figcaption>{}</figcaption>", HtmlEscape(caption))?;
                    }
                    write!(w, "</figure>")?;
                } else {
                    if path.starts_with("http://") || path.starts_with("https://") {
                        attrs.push_str(r#" target="_blank" rel="noopener""#);
                    }

                    write!(
                        w,
                        "<a href=\"{}\" {attrs}>{}</a>",
                        HtmlEscape(url.as_str()),
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
            Element::SourceBlock(block) => {
                if block.language.is_empty() {
                    write!(w, "<pre class=\"example\">{}</pre>", block.contents)?;
                } else {
                    let highlight = if block.language.eq_ignore_ascii_case("php")
                        && !block.contents.starts_with("<?php")
                    {
                        highlight(
                            &self.handler,
                            Some(&block.language),
                            &format!("<?php\n{}", block.contents),
                        )
                    } else {
                        highlight(&self.handler, Some(&block.language), &block.contents)
                    };

                    write!(
                        w,
                        "<div class=\"org-src-container\"><pre class=\"src src-{}\">{}</pre></div>",
                        block.language, highlight
                    )?;
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

// from https://docs.rs/orgize/latest/src/orgize/export/html.rs.html#330
fn highlight<E: From<Error>, H: HtmlHandler<E>>(
    syntect: &SyntectHtmlHandler<E, H>,
    language: Option<&str>,
    content: &str,
) -> String {
    let mut highlighter = HighlightLines::new(
        language
            .and_then(|lang| syntect.syntax_set.find_syntax_by_token(lang))
            .unwrap_or_else(|| syntect.syntax_set.find_syntax_plain_text()),
        &syntect.theme_set.themes[&syntect.theme],
    );

    let regions = highlighter.highlight(content, &syntect.syntax_set);
    styled_line_to_highlighted_html(&regions[..], syntect.background)
}

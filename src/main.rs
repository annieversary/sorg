use color_eyre::{Report, Result};
use orgize::{Headline, Org, ParseConfig};
use slugmin::slugify;
use std::{
    borrow::Cow,
    collections::HashMap,
    fs::File,
    io::{Read, Write},
    path::Path,
};
use tera::{Context, Tera};

fn main() -> Result<(), Report> {
    color_eyre::install()?;

    let tera = Tera::new("templates/*.html")?;

    let mut f = File::open("./blog.org")?;
    let mut src = String::new();
    f.read_to_string(&mut src)?;

    let org = Org::parse_custom(
        &src,
        &ParseConfig {
            todo_keywords: (
                vec![
                    "TODO".to_string(),
                    "PROGRESS".to_string(),
                    "WAITING".to_string(),
                    "MAYBE".to_string(),
                ],
                vec!["DONE".to_string(), "CANCELLED".to_string()],
            ),
            ..Default::default()
        },
    );
    let doc = org.document();

    let first = doc.first_child(&org).unwrap();

    render_index(&org, &first, &tera, "./build")?;

    Ok(())
}

fn render_index<'a>(org: &Org<'a>, headline: &Headline, tera: &Tera, out: &str) -> Result<()> {
    let title = headline.title(&org);
    let properties = headline.title(&org).properties.clone().into_hash_map();
    let name = &title.raw;

    let template = get_template(&properties, name);
    let out_path = get_out(&properties, name, out);
    let context = get_index_context(headline, org);

    render_template(&tera, &template, &context, &out_path)?;

    // render subpages
    for page in headline.children(&org) {
        if page.title(&org).tags.contains(&Cow::Borrowed("post")) {
            render_post(&org, &page, tera, &out_path)?;
        } else {
            render_index(&org, &page, tera, &out_path)?;
        }
    }
    Ok(())
}

fn render_post<'a>(org: &Org<'a>, headline: &Headline, tera: &Tera, out: &str) -> Result<()> {
    let title = headline.title(&org);
    let properties = headline.title(&org).properties.clone().into_hash_map();
    let name = &title.raw;

    let template = get_template(&properties, name);
    let out_path = get_out(&properties, name, out);
    let context = get_post_context(headline, org);

    render_template(&tera, &template, &context, &out_path)?;

    Ok(())
}

fn get_template<'a>(
    properties: &HashMap<Cow<'a, str>, Cow<'a, str>>,
    name: &Cow<'a, str>,
) -> Cow<'a, str> {
    if let Some(template) = properties.get("template") {
        // use $name.html as a template
        template.clone()
    } else if Path::new(&format!("{name}.html")).exists() {
        // use template
        Cow::Owned(format!("{name}.html"))
    } else {
        Cow::Borrowed("default.html")
    }
}

fn get_out<'a>(
    properties: &HashMap<Cow<'a, str>, Cow<'a, str>>,
    name: &Cow<'a, str>,
    out: &str,
) -> String {
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

fn get_index_context<'a>(headline: &Headline, org: &Org<'a>) -> Context {
    let pages = headline
        .children(&org)
        .map(|h| h.title(&org).raw.to_owned())
        .map(|h| (slugify(&h), h))
        .collect::<Vec<_>>();

    let mut context = Context::new();
    context.insert("title", &"name");
    context.insert("content", &"heyyy");
    context
}
fn get_post_context<'a>(headline: &Headline, org: &Org<'a>) -> Context {
    let sections = headline
        .children(&org)
        .map(|h| h.title(&org).raw.to_owned())
        .collect::<Vec<_>>();

    let mut context = Context::new();
    context.insert("title", &"name");
    context.insert("content", &"heyyy");
    context
}

fn render_template(tera: &Tera, template: &str, context: &Context, out_path: &str) -> Result<()> {
    let content = tera.render(template, context)?;

    std::fs::create_dir_all(out_path)?;
    let path = format!("{out_path}/index.html");

    // TODO save it as out_path/index.html
    let mut file = File::create(path)?;
    file.write_all(content.as_bytes())?;

    Ok(())
}

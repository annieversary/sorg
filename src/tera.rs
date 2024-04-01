use serde::Serialize;
use std::{borrow::Cow, collections::HashMap};
use tera::{to_value, Tera, Value};

use crate::{
    config::Config,
    page::{Page, PageEnum},
};

pub fn make_tera(config: &Config) -> Result<Tera, tera::Error> {
    let mut template_folder_path = config.root_folder.clone();
    template_folder_path.push(
        config
            .preamble
            .get("templates")
            .map(|a| a.as_str())
            .unwrap_or("templates"),
    );
    Tera::new(&format!(
        "{}/*.html",
        template_folder_path.to_string_lossy()
    ))
}

pub fn make_get_pages(root: &'_ Page<'_>) -> impl tera::Function {
    let mut map = HashMap::new();

    add(root, &mut map);

    Box::new(
        move |args: &HashMap<String, Value>| -> tera::Result<Value> {
            match args.get("path") {
                Some(val) => match tera::from_value::<String>(val.clone()) {
                    Ok(path) => {
                        let mut o = vec![];
                        for (k, v) in &map {
                            if k.starts_with(&path) {
                                o.push(v);
                            }
                        }

                        let o = to_value(o).unwrap();
                        Ok(o)
                    }
                    // Ok(v) => Ok(to_value(urls.get(&v).unwrap()).unwrap()),
                    Err(_) => Err("oops".into()),
                },
                None => Err("oops".into()),
            }
        },
    )
}

#[derive(Serialize, Debug)]
struct Link {
    link: String,
    title: String,
    closed_at: Option<String>,
    description: Option<String>,
    order: usize,
}

fn add(page: &Page<'_>, map: &mut HashMap<String, Link>) {
    map.insert(
        page.path.clone(),
        Link {
            link: page.path.clone(),
            title: page.info.title.clone(),
            description: page.info.description.to_owned(),
            order: page.order,
            closed_at: page
                .info
                .closed_at
                .as_ref()
                .map(|d| format!("{}-{:0>2}-{:0>2}", d.year, d.month, d.day)),
        },
    );
    if let PageEnum::Index { children } = &page.page {
        for child in children.values() {
            add(child, map);
        }
    }
}

/// get the correct template to use for a page
///
/// `template` property, `{name}.html`, or `default.html`
pub fn get_template<'a>(
    tera: &Tera,
    name: Option<&'a String>,
    path: &str,
    index: bool,
) -> Cow<'a, str> {
    let path = if path == "/" {
        "index"
    } else {
        path.trim_start_matches('/')
    };

    // use template set in properties
    if let Some(template) = name {
        template.into()
    }
    // use $name.html as a template
    else if tera
        .get_template_names()
        .any(|x| x == format!("{path}.html"))
    {
        Cow::Owned(format!("{path}.html"))
    } else if index {
        Cow::Borrowed("default_index.html")
    }
    // use default.html
    else {
        Cow::Borrowed("default.html")
    }
}

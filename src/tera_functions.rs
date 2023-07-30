use serde::Serialize;
use std::collections::HashMap;
use tera::{to_value, Value};

use crate::page::{Page, PageEnum};

pub fn make_get_pages(root: &'_ Page<'_>) -> impl tera::Function {
    let mut map = HashMap::new();

    add(root, &mut map, "".to_string());

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
}

fn add(page: &Page<'_>, map: &mut HashMap<String, Link>, mut path: String) {
    if page.slug != "index" {
        path = format!("{path}/{}", page.slug);
    }

    let p = if path.is_empty() {
        "/".to_string()
    } else {
        path.clone()
    };

    map.insert(
        p.clone(),
        Link {
            link: p,
            title: page.title.clone(),
        },
    );
    if let PageEnum::Index { children } = &page.page {
        for child in children.values() {
            add(child, map, path.clone());
        }
    }
}

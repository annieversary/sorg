use rss::{
    extension::atom::{self, AtomExtension, Link},
    *,
};

use crate::{config::Config, page::Page};

pub fn generate_rss(
    children: Vec<(&Page<'_>, tera::Context)>,
    config: &Config,
    path: &str,
) -> String {
    let mut items = Vec::with_capacity(children.len());
    for (page, context) in children {
        items.push(
            ItemBuilder::default()
                .title(Some(page.title.clone()))
                .link(Some(format!("{}{}", config.url, page.path)))
                .guid(Some(Guid {
                    value: format!("{}{}", config.url, page.path),
                    permalink: true,
                }))
                .pub_date(
                    page.closed_at
                        .as_ref()
                        .map(|d| -> chrono::NaiveDateTime { d.into() })
                        .map(|d| d.format("%a, %d %b %Y %H:%M:%S GMT").to_string()),
                    // .map(|d| d.format("%a, %d %b %Y %H:%M:%S GMT")),
                )
                .description(
                    page.description
                        .clone()
                        .or_else(|| Some(config.description.clone())),
                )
                .content(
                    context
                        .get("content")
                        .and_then(|a| a.as_str())
                        .map(ToString::to_string),
                )
                .build(),
        );
    }

    let mut atom = AtomExtension::default();
    atom.set_links([Link {
        href: format!("{}{}/rss.xml", config.url, path),
        rel: "self".to_string(),
        ..Default::default()
    }]);

    let channel = ChannelBuilder::default()
        .namespaces([("atom".to_string(), atom::NAMESPACE.to_string())])
        .title(&config.title)
        .link(&config.url)
        .atom_ext(Some(atom))
        .description(&config.description)
        .items(items)
        .build();

    channel.to_string()
}

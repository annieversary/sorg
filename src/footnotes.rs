use orgize::{
    elements::{FnDef, FnRef},
    indextree::NodeEdge,
    Element, Event, Headline, Org,
};
use serde_derive::Serialize;

#[derive(Serialize)]
pub struct Footnote {
    label: String,
    definition: String,
}

pub fn get_footnotes(org: &Org<'_>, headline: &Headline) -> Vec<Footnote> {
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

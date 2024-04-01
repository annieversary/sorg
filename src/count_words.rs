use orgize::{elements::Link, indextree::NodeEdge, Element, Event, Headline, Org};

pub fn count_words_index(headline: &Headline, org: &Org<'_>) -> usize {
    // dont count children headlines, just the actual text on this page
    let it = headline
        .headline_node()
        .traverse(org.arena())
        .map(move |edge| match edge {
            NodeEdge::Start(node) => Event::Start(&org[node]),
            NodeEdge::End(node) => Event::End(&org[node]),
        });

    let mut v = 0;
    let mut in_subheadline = false;
    let mut in_headline = false;
    let mut in_page_title = false;

    for event in it {
        // println!("sub: {in_subheadline}, head: {in_headline}, title: {in_page_title}");
        match event {
            Event::Start(element) if !in_headline => match element {
                Element::Headline { level } if *level > headline.level() + 1 => {
                    in_headline = true;
                }
                Element::Headline { level } if *level > headline.level() => {
                    in_subheadline = true;
                }
                Element::Title(_) => {
                    in_page_title = true;
                }
                _ if !in_subheadline || in_page_title => {
                    // count this element
                    v += match element {
                        Element::Text { value } => words_count::count(value).words,
                        Element::Link(Link {
                            desc: Some(value), ..
                        }) => words_count::count(value).words,
                        _ => 0,
                    };
                }
                // while we are in a title, we'll land here, cause we dont want to show the child posts
                _ => {}
            },
            Event::End(element) => match element {
                Element::Headline { level } if *level > headline.level() + 1 => {
                    in_headline = false;
                }
                Element::Headline { level } if *level > headline.level() => {
                    in_subheadline = false;
                }
                Element::Title(_) => {
                    in_page_title = false;
                }
                _ => {}
            },
            _ => {}
        }
    }

    v
}
pub fn count_words_post(headline: &Headline, org: &Org<'_>) -> usize {
    headline
        .headline_node()
        .traverse(org.arena())
        .flat_map(move |edge| match edge {
            NodeEdge::Start(node) => Some(&org[node]),
            NodeEdge::End(_) => None,
        })
        .map(|el| match el {
            Element::Text { value } => words_count::count(value).words,
            Element::Link(Link {
                desc: Some(value), ..
            }) => words_count::count(value).words,
            _ => 0,
        })
        .sum()
}

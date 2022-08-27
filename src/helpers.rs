use orgize::{Element, Event, Org};

/// extracts a file out of an org link
///
/// "[[file:test.org][linked blogpost]]" -> "test.org"
pub fn parse_file_link(link: &str) -> Option<String> {
    let org = Org::parse(link);

    let mut it = org.iter();

    if let Some(Event::Start(Element::Document { .. })) = it.next() {
        if let Some(Event::Start(Element::Section)) = it.next() {
            if let Some(Event::Start(Element::Paragraph { .. })) = it.next() {
                if let Some(Event::Start(Element::Link(link))) = it.next() {
                    if let Some(p) = link.path.strip_prefix("file:") {
                        return Some(p.to_string());
                    }
                }
            }
        }
    }

    // TODO maybe parse other stuff here

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_file_link() {
        let s = parse_file_link("[[file:test.org][linked blogpost]]");
        assert_eq!(s, Some("test.org".to_string()));
    }
}

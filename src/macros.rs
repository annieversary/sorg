use std::collections::HashMap;

use color_eyre::{eyre::bail, Result};
use orgize::{Element, Org};

#[derive(Debug, PartialEq, Clone)]
pub struct Macro {
    label: String,
    arguments: Vec<String>,
    definition: String,
}

#[derive(Default)]
pub struct Macros {
    macros: HashMap<String, Macro>,
}

impl Macros {
    pub fn parse<'a>(org: &'a Org<'a>) -> Self {
        let mut macros = HashMap::default();
        let mut in_macro: Option<Macro> = None;

        for ev in org.iter() {
            match ev {
                orgize::Event::Start(element) => match element {
                    Element::SpecialBlock(block) => {
                        if let Some(parameters) = &block.parameters {
                            let mut parameters = parameters.split_whitespace();

                            let Some(label) = parameters.next() else {
                                continue;
                            };

                            in_macro = Some(Macro {
                                label: label.to_string(),
                                arguments: parameters.map(ToString::to_string).collect(),
                                definition: "".to_string(),
                            });
                        }
                    }
                    Element::Text { value } if in_macro.is_some() => {
                        if let Some(r#macro) = &mut in_macro {
                            r#macro.definition.push_str(value);
                        }
                    }

                    _ => {}
                },
                orgize::Event::End(element) => {
                    if let Element::SpecialBlock(_) = element {
                        if let Some(r#macro) = in_macro.take() {
                            macros.insert(r#macro.label.clone(), r#macro);
                        }
                    }
                }
            }
        }

        Self::from_macros(macros)
    }

    pub fn from_macros(macros: HashMap<String, Macro>) -> Self {
        Self { macros }
    }

    pub fn get<'a>(&'a self, name: &'_ str) -> Option<MacroProcessor<'a>> {
        self.macros
            .get(name)
            .map(|definition| MacroProcessor { definition })
    }
}

pub struct MacroProcessor<'a> {
    definition: &'a Macro,
}

impl<'a> MacroProcessor<'a> {
    pub fn process(&self, arguments: &str) -> Result<String> {
        let arguments = arguments
            .split(',')
            .map(|arg| arg.trim())
            .collect::<Vec<_>>();

        if arguments.len() != self.definition.arguments.len() {
            bail!(
                "macro call argument count mismatch for {}",
                self.definition.label
            );
        }

        // TODO this should be tera instead
        let mut content = self.definition.definition.clone();
        for (name, value) in self.definition.arguments.iter().zip(arguments.iter()) {
            content = content.replace(&format!("${name}"), value)
        }

        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_parse_macro_definition() {
        let source = "#+BEGIN_MACRO test $name $surname
hello $name $surname
#+END_MACRO";

        let org = Org::parse(source);

        let macros = Macros::parse(&org);

        assert_eq!(
            macros.macros.get("test").cloned(),
            Some(Macro {
                label: "test".to_string(),
                arguments: vec!["$name".to_string(), "$surname".to_string()],
                definition: "hello $name $surname".to_string()
            })
        );
    }

    #[test]
    fn can_parse_multiline_macro() {
        let source = "#+BEGIN_MACRO test $name $surname
hello $name $surname
this is a second line
#+END_MACRO";

        let org = Org::parse(source);

        let macros = Macros::parse(&org);

        assert_eq!(
            macros.macros.get("test").cloned(),
            Some(Macro {
                label: "test".to_string(),
                arguments: vec!["$name".to_string(), "$surname".to_string()],
                definition: "hello $name $surname\nthis is a second line".to_string()
            })
        );
    }

    #[test]
    fn can_call_macro() {
        let m = Macro {
            label: "test".to_string(),
            arguments: vec!["arg1".to_string(), "arg2".to_string()],
            definition: "hello $arg1 hey $arg2".to_string(),
        };

        let m = MacroProcessor { definition: &m };

        assert_eq!("hello name1 hey name2", m.process("name1, name2").unwrap());
    }

    #[test]
    fn can_call_macro_argument_count_mismatch() {
        let m = Macro {
            label: "test".to_string(),
            arguments: vec!["arg1".to_string(), "arg2".to_string()],
            definition: "hello $arg1 hey $arg2".to_string(),
        };

        let m = MacroProcessor { definition: &m };

        assert!(m.process("name1, name2, name3").is_err());
    }
}

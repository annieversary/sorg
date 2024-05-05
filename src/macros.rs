use std::{collections::HashMap, fmt::Write};

use color_eyre::{eyre::bail, Result};
use orgize::{Element, Org};
use tera::Tera;

#[derive(Debug, PartialEq, Clone)]
pub struct Macro {
    label: String,
    arguments: Vec<String>,
    definition: String,
}

impl Macro {
    fn to_tera_macro(&self, mut out: impl Write) -> Result<()> {
        out.write_str("{% macro ")?;
        out.write_str(&self.label)?;
        out.write_str("(")?;
        for arg in self.arguments.iter().flat_map(|a| [", ", a]).skip(1) {
            out.write_str(arg)?;
        }
        out.write_str(") %}")?;
        out.write_str(&self.definition)?;
        out.write_str("{% endmacro ")?;
        out.write_str(&self.label)?;
        out.write_str(" %}")?;

        Ok(())
    }
}

#[derive(Default)]
pub struct Macros {
    macros: HashMap<String, Macro>,
    tera: Tera,
}

impl Macros {
    pub fn parse<'a>(org: &'a Org<'a>) -> Result<Self> {
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

    pub fn from_macros(macros: HashMap<String, Macro>) -> Result<Self> {
        let mut tera = Tera::default();

        // there's no register_macro function, so we need to make a template that contains all the macro definitions
        // we can then import this template in all the actual templates
        let mut macros_template = String::new();
        for m in macros.values() {
            m.to_tera_macro(&mut macros_template)?;
        }
        tera.add_raw_template("__all_macro_definitions", &macros_template)?;

        for m in macros.values() {
            let mut template = r#"{% import "__all_macro_definitions" as macros %}"#.to_string();
            template.push_str(&m.definition);

            tera.add_raw_template(&m.label, &template)?;
        }

        tera.build_inheritance_chains()?;

        Ok(Self { macros, tera })
    }

    pub fn get<'a>(&'a self, name: &'_ str) -> Option<MacroProcessor<'a>> {
        self.macros.get(name).map(|definition| MacroProcessor {
            definition,
            tera: &self.tera,
        })
    }
}

pub struct MacroProcessor<'a> {
    definition: &'a Macro,
    tera: &'a Tera,
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

        let mut context = tera::Context::new();
        for (name, value) in self.definition.arguments.iter().zip(arguments.iter()) {
            context.insert(name, value);
        }

        let content = self.tera.render(&self.definition.label, &context)?;

        Ok(content)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_parse_macro_definition() {
        let source = "#+BEGIN_MACRO test name surname
hello {{ name }} {{ surname }}
#+END_MACRO";

        let org = Org::parse(source);

        let macros = Macros::parse(&org).unwrap();

        assert_eq!(
            macros.macros.get("test").cloned(),
            Some(Macro {
                label: "test".to_string(),
                arguments: vec!["name".to_string(), "surname".to_string()],
                definition: "hello {{ name }} {{ surname }}".to_string()
            })
        );
    }

    #[test]
    fn can_parse_multiline_macro() {
        let source = "#+BEGIN_MACRO test name surname
hello {{ name }} {{ surname }}
this is a second line
#+END_MACRO";

        let org = Org::parse(source);

        let macros = Macros::parse(&org).unwrap();

        assert_eq!(
            macros.macros.get("test").cloned(),
            Some(Macro {
                label: "test".to_string(),
                arguments: vec!["name".to_string(), "surname".to_string()],
                definition: "hello {{ name }} {{ surname }}\nthis is a second line".to_string()
            })
        );
    }

    #[test]
    fn to_tera_macro() {
        let m = Macro {
            label: "test".to_string(),
            arguments: vec!["arg1".to_string(), "arg2".to_string()],
            definition: "hello {{ arg1 }} hey {{ arg2 }}".to_string(),
        };

        const RES: &str =
            "{% macro test(arg1, arg2) %}hello {{ arg1 }} hey {{ arg2 }}{% endmacro test %}";

        let mut out = String::new();
        m.to_tera_macro(&mut out).unwrap();
        assert_eq!(RES, out);
    }

    #[test]
    fn can_call_macro() {
        let m = Macro {
            label: "test".to_string(),
            arguments: vec!["arg1".to_string(), "arg2".to_string()],
            definition: "hello {{ arg1 }} hey {{ arg2 }}".to_string(),
        };

        let macros = Macros::from_macros([("test".to_string(), m)].into_iter().collect()).unwrap();

        assert_eq!(
            "hello name1 hey name2",
            macros.get("test").unwrap().process("name1, name2").unwrap()
        );
    }

    #[test]
    fn can_call_macro_argument_count_mismatch() {
        let m = Macro {
            label: "test".to_string(),
            arguments: vec!["arg1".to_string(), "arg2".to_string()],
            definition: "hello {{ arg1 }} hey {{ arg2 }}".to_string(),
        };

        let macros = Macros::from_macros([("test".to_string(), m)].into_iter().collect()).unwrap();

        assert!(macros
            .get("test")
            .unwrap()
            .process("name1, name2, name3")
            .is_err());
    }

    #[test]
    fn can_call_macro_from_inside_macro() {
        let macro1 = Macro {
            label: "macro1".to_string(),
            arguments: vec!["arg1".to_string()],
            definition: "hello {{ macros::macro2(name=arg1) }}".to_string(),
        };
        let macro2 = Macro {
            label: "macro2".to_string(),
            arguments: vec!["name".to_string()],
            definition: "my name is {{ name }}".to_string(),
        };

        let macros = Macros::from_macros(
            [
                ("macro1".to_string(), macro1),
                ("macro2".to_string(), macro2),
            ]
            .into_iter()
            .collect(),
        )
        .unwrap();

        assert_eq!(
            "hello my name is annie",
            macros.get("macro1").unwrap().process("annie").unwrap()
        );
    }
}

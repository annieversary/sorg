use color_eyre::Result;
use std::{borrow::Cow, collections::HashMap, fs::File, io::Write};
use tera::{Context, Tera};

pub fn get_template<'a>(
    tera: &Tera,
    properties: &HashMap<Cow<'a, str>, Cow<'a, str>>,
    name: &str,
) -> Cow<'a, str> {
    // use template set in properties
    if let Some(template) = properties.get("template") {
        template.clone()
    }
    // use $name.html as a template
    else if tera
        .get_template_names()
        .any(|x| x == format!("{name}.html"))
    {
        Cow::Owned(format!("{name}.html"))
    }
    // use default.html
    else {
        Cow::Borrowed("default.html")
    }
}

pub fn render_template(
    tera: &Tera,
    template: &str,
    context: &Context,
    out_path: &str,
) -> Result<()> {
    let content = tera.render(template, context)?;

    std::fs::create_dir_all(out_path)?;
    let path = format!("{out_path}/index.html");

    let mut file = File::create(path)?;
    file.write_all(content.as_bytes())?;

    Ok(())
}

use color_eyre::Result;
use std::{borrow::Cow, collections::HashMap, io::Write};
use tera::{Context, Tera};
use vfs::VfsPath;

/// get the correct template to use for a page
///
/// `template` property, `{name}.html`, or `default.html`
pub fn get_template<'a>(
    tera: &Tera,
    properties: &HashMap<Cow<'a, str>, Cow<'a, str>>,
    path: &str,
    index: bool,
) -> Cow<'a, str> {
    let path = if path == "/" {
        "index"
    } else {
        path.trim_start_matches('/')
    };

    // use template set in properties
    if let Some(template) = properties.get("template") {
        template.clone()
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

/// renders the given template to the output path using the provided context
pub fn render_template(
    tera: &Tera,
    template: &str,
    context: &Context,
    out_path: VfsPath,
    hotreloading: bool,
) -> Result<String> {
    let mut content = tera.render(template, context)?;

    if hotreloading {
        content.push_str("<script>(() => { const socket = new WebSocket('ws://localhost:2794', 'sorg'); socket.addEventListener('message', () => {location.reload();}); })();</script>",);
    }

    out_path.create_dir_all()?;

    let mut file = out_path.join("index.html")?.create_file()?;
    file.write_all(content.as_bytes())?;

    Ok(content)
}

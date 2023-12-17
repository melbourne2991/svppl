use anyhow::Result;
use async_trait::async_trait;
use futures::stream::Stream;
use futures::StreamExt;
use handlebars::Handlebars;
use serde_json::json;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::pin::Pin;

#[async_trait]
pub (crate) trait ProjectTemplateSource {
    async fn get_templates(&self) -> Pin<Box<dyn Stream<Item = Result<(String, String)>>>>;
}

pub (crate) struct DefaultProjectTemplateSource {}

const TMPL_PACKAGE_JSON: &[u8] = include_bytes!("templates/typescript/package.json.hbs");

#[async_trait]
impl ProjectTemplateSource for DefaultProjectTemplateSource {
    async fn get_templates(&self) -> Pin<Box<dyn Stream<Item = Result<(String, String)>>>> {
        let stream = futures::stream::iter(vec![Ok((
            "package.json".to_string(),
            std::str::from_utf8(TMPL_PACKAGE_JSON)
                .expect("File was not valid UTF-8")
                .to_string(),
        ))]);
        Box::pin(stream)
    }
}

pub (crate) struct ProjectTemplateOptions {
    pub target_dir: String,
    pub package_name: String,
}

pub (crate) async fn render<T: ProjectTemplateSource>(
    source: &T,
    options: ProjectTemplateOptions,
) -> Result<Vec<PathBuf>> {
    let mut reg = Handlebars::new();
    reg.register_template_string("tpl_1", "Good afternoon, {{name}}")?;

    let mut templates_stream = source.get_templates().await;

    let mut wrote_paths = vec![];

    while let Some(template) = templates_stream.next().await {
        let (file_path, content) = template?;
        let data = json!({"package_name": options.package_name});

        let full_path = Path::new(options.target_dir.as_str()).join(file_path.as_str());
        let mut file = File::create(full_path.clone())?;

        reg.render_template_to_write(&content, &data, &mut file)?;

        wrote_paths.push(full_path);
    }

    Ok(wrote_paths)
}

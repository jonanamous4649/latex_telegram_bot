use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use tera::{Context as TeraContext, Tera};
use tokio::fs as tokio_fs;
use tokio::process::Command as TokioCommand;

pub struct LatexRenderer {
    tera: Tera,
    output_dir: PathBuf,
    latex_cmd: String,
}

impl LatexRenderer {

    // function called 'new()'
    // &Path is borrowed since we only reference
    // use Result<Self> for typical file operation failures
    pub fn new(output_dir: &Path) -> Result<Self> {

        // creates new directory at &Path
        std::fs::create_dir_all(output_dir)?;

        // inialize tera template engine
        // Tera::new() takes a glob pattern for template files
        // 'templates/**/*/' means: templates folder, all subfolders, all files
        let tera = match Tera::new("templates/**/*") {
            Ok(t) => t,
            Err(_) => {
                Tera::default()
            }
        };

        Ok(LatexRenderer {
            tera,
            output_dir: output_dir.to_path_buf(), // convert &Path to owned
            latex_cmd: "pdflatex".to_string(),
        })
    }

    pub async fn render(
        &self,
        template_name: &str,
        context: TeraContext,
        output_name: &str,
    ) -> Result<PathBuf> {  // Returns path to generated PDF

        // ensure template dir exists
        // Path::new() creates a path from string
        let template_dir = Path::new("templates");
        if !template_dir.exists() {
            println!(" Creating default tempalte...");
            self.create_default_template().await?;
        }

        // reload tera to puck up newly created templates
        // fresh tera instance to load templates
        let mut tera = Tera::new("templates/**/*")
            .context("Failed to load templates")?;

        // disable auto-escaping for .tex files (issue with backslahes)
        tera.autoescape_on(vec!["html", "htm", "xml"]);

        // render template with data
        println!(" Filling template with data...");
        let latex_content = tera.render(template_name, &context)
            .with_context(|| {
                format!("Failed to render template: {}", template_name)
            })?;

        Ok(pdf_path)
    }
}
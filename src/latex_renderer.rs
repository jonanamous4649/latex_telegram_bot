use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tera::{Context as TeraContext, Tera};
use tokio::fs as tokio_fs;
use tokio::process::Command as TokioCommand;

pub struct LatexRenderer {
    output_dir: PathBuf,
}

impl LatexRenderer {

    // function called 'new()'
    // &Path is borrowed since we only reference
    // use Result<Self> for typical file operation failures
    pub fn new(output_dir: &Path) -> Result<Self> {

        // creates new directory at &Path
        std::fs::create_dir_all(output_dir)?;

        Ok(LatexRenderer {
            output_dir: output_dir.to_path_buf(), // convert &Path to owned
        })
    }

    pub async fn render(
        &self,
        template_name: &str,
        context: TeraContext,
        output_name: &str,
    ) -> Result<PathBuf> {  // Returns path to generated PDF

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

        // write .tex to disk and join paths for full file location
        let tex_path = self.output_dir.join(format!("{}.tex", output_name));
        println!(" Writing {}...", tex_path.display());

        // tokio_fs::write is async. await pauses here until write complete
        // ? returns err if fail
        tokio_fs::write(&tex_path, latex_content).await
            .with_context(|| format!("failed to write {}", tex_path.display()))?;

        // compile LaTeX to PDF. runs twice to resolve cross-references
        println!(" Running LaTeX (pass 1/2)...");
        self.run_latex(&tex_path).await?;

        println!(" Running LaTeX (pass 2/2)...");
        self.run_latex(&tex_path).await?;

        // verify PDF created
        let pdf_path = self.output_dir.join(format!("{}.pdf", output_name));
        if !pdf_path.exists() {
            anyhow::bail!("PDF file was not created! Check LaTeX errors above.");
        }

        Ok(pdf_path)
    }

    async fn run_latex(&self, tex_path: &Path) -> Result<()> {

        // TokioCommand is async version of std::process::Command
        // doesn't black thread while waiting for LaTeX to finish
        let output = TokioCommand::new("pdflatex".to_string())
            .args(&[
                "-interaction=nonstopmode", // don't stop for user input on errors
                "-halt-on-error",           // exit immediately on first error
                "-output-directory",        // specify where to put output files
                &self.output_dir.to_string_lossy(), // convert to PathBuf to string
                &tex_path.to_string_lossy(),        // convert to Path to string
            ])
            .output()
            .await
            .with_context(|| format!("Failed to run {}", "pdflatex".to_string()))?;

        // check exit status. '.success()' returns true if exit code was 0 (success)
        if !output.status.success() {
            // convert bytes to string to make error reproting smooth
            let stderr = String::from_utf8_lossy(&output.stderr);
            let stdout = String::from_utf8_lossy(&output.stdout);

            // print
            eprintln!("LaTeX STDOUT:\n{}", stdout);
            eprintln!("LaTeX STDERR:\n{}", stderr);

            anyhow::bail!("LaTeX compliation failed. See output above.");
        }

        Ok(())
    }

}
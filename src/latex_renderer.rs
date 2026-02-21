use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tera::{Context as TeraContext, Tera};
use tokio::fs as tokio_fs;
use tokio::process::Command as TokioCommand;

pub struct LatexRenderer {
    tera: Tera,
    output_dir: PathBuf,
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

    // create default template if not exists
    async fn create_default_template(&self) -> Result<()> {

        // create directory for template
        tokio_fs::create_dir_all("templates").await
            .context("failed to create tempaltes directory")?;

        // template content as raw string
        // r# syntax: r = raw with # delimiters
        let template_content = r#"
\documentclass[11pt]{article}
\usepackage[a4paper, top=70pt, bottom=30pt, left=0.5in, right=0.5in]{geometry}
\usepackage[utf8]{inputenc}
\usepackage{amsmath, booktabs}
\usepackage[scaled]{helvet}
\usepackage{xcolor}
\usepackage{eso-pic}
\usepackage{fancyhdr}

% Header box ↓
\definecolor{headerblue}{RGB}{0, 51, 102}

\AddToShipoutPictureBG{%
  \AtPageUpperLeft{%
    \raisebox{-60pt}{\color{headerblue}\rule{\paperwidth}{60pt}}%
  }%
}

\setlength{\headheight}{60pt}
\addtolength{\topmargin}{-14.32pt}

\AddToShipoutPictureFG{%
  \AtPageUpperLeft{%
    \raisebox{-38pt}{\makebox[\paperwidth][c]{\parbox[c]{\paperwidth}{\centering\textcolor{white}{%
        \Large\textbf{{ report_title }}\\[2pt]%
        \small { generation_date }}}}}%
  }% 
}

\pagestyle{fancy}
\fancyhf{}
\setlength{\headheight}{45.68002pt}
\addtolength{\topmargin}{-4.08003pt}
\renewcommand{\headrulewidth}{0pt}
% Header box ↑

% Footer box ↓
\AddToShipoutPictureBG{%
  \AtPageLowerLeft{%
    \raisebox{0pt}{\color{headerblue}\rule{\paperwidth}{20pt}}%
  }%
}

\AddToShipoutPictureFG{%
  \AtPageLowerLeft{%
    \raisebox{6pt}{\makebox[\paperwidth][c]{\textcolor{white}{My Footer}}}%
  }%
}
% Footer box ↑

\begin{document}

\section{Data Summary}
\begin{itemize}
    {% for metric in metrics %}
    \item \textbf{ {{ metric.name }} }: {{ metric.value }} {{ metric.unit }}
    {% endfor %}
\end{itemize}

\section{Analysis}
{{ analysis_text }}

{% if include_table %}
\section{Data Table}
\begin{center}
\begin{tabular}{ {{ table_columns | join(" ") }} }
\toprule
{% for row in table_data %}
    {{ row | join(" & ") }} \\
{% endfor %}
\bottomrule
\end{tabular}
\end{center}
{% endif %}

\end{document}
"#;

        // write template file
        tokio_fs::write("templates/template.tex", template_content).await
            .context("Failed to write default demplate")?;

        println!(" Default template created at templates/template.tex");

        Ok(())
    }

}
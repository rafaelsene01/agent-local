use std::path::Path;

#[derive(Debug)]
pub enum ParseError {
    /// The file opened fine but holds no extractable text — a scanned PDF is
    /// the usual case. Distinct from a read failure because the user needs a
    /// different answer (OCR isn't supported) than "the file is broken".
    NoTextFound,
    UnsupportedFormat(String),
    ReadFailed(String),
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::NoTextFound => write!(
                f,
                "nenhum texto encontrado neste arquivo (PDFs digitalizados precisam de OCR, ainda não suportado)"
            ),
            ParseError::UnsupportedFormat(ext) => {
                write!(f, "formato não suportado: .{ext}")
            }
            ParseError::ReadFailed(msg) => write!(f, "não foi possível ler o arquivo: {msg}"),
        }
    }
}

impl std::error::Error for ParseError {}

pub const SUPPORTED_EXTENSIONS: [&str; 4] = ["pdf", "docx", "txt", "md"];

pub fn extension_of(path: &Path) -> String {
    path.extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

pub fn is_supported(path: &Path) -> bool {
    SUPPORTED_EXTENSIONS.contains(&extension_of(path).as_str())
}

/// PDFs go through pdfium (see `rag::pdfium` for why `pdf-extract` was
/// dropped); `docx-rs` 0.4.22 handles DOCX. `dotext`, the other DOCX candidate
/// named in the design, was rejected — its last release is from 2017.
pub fn extract_text(path: &Path) -> Result<String, ParseError> {
    let text = match extension_of(path).as_str() {
        "txt" | "md" => {
            std::fs::read_to_string(path).map_err(|e| ParseError::ReadFailed(e.to_string()))?
        }
        "pdf" => extract_pdf(path)?,
        "docx" => extract_docx(path)?,
        other => return Err(ParseError::UnsupportedFormat(other.to_string())),
    };

    if text.trim().is_empty() {
        return Err(ParseError::NoTextFound);
    }
    Ok(text)
}

fn extract_pdf(path: &Path) -> Result<String, ParseError> {
    let text = super::pdfium::extract_text(path).map_err(ParseError::ReadFailed)?;
    Ok(rejoin_hyphenated_words(&text))
}

/// A PDF breaks words across lines, and the extractor turns that break into
/// "liqui- dação" or "empre- sário". Left alone, those halves are what gets
/// embedded and what the model reads — seen in the user's Civil Code import.
///
/// Only a hyphen *followed by a space and a lowercase letter* is joined: a
/// real Portuguese hyphen ("far-se-á", "guarda-chuva") has no space after it,
/// and a lowercase continuation is what a broken word looks like.
fn rejoin_hyphenated_words(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    while i < chars.len() {
        let is_break = chars[i] == '-'
            && i > 0
            && chars[i - 1].is_alphabetic()
            && chars.get(i + 1).is_some_and(|c| *c == ' ' || *c == '\n')
            && chars
                .get(i + 2)
                .is_some_and(|c| c.is_alphabetic() && c.is_lowercase());
        if is_break {
            i += 2; // drop the hyphen and the whitespace
        } else {
            out.push(chars[i]);
            i += 1;
        }
    }
    out
}

/// docx-rs exposes the document as a tree of nodes; the text lives in the
/// `Text` leaves, so the tree is walked instead of guessing at the XML.
fn extract_docx(path: &Path) -> Result<String, ParseError> {
    use docx_rs::{DocumentChild, ParagraphChild, RunChild, TableCellContent, TableChild, TableRowChild};

    let bytes = std::fs::read(path).map_err(|e| ParseError::ReadFailed(e.to_string()))?;
    let docx = docx_rs::read_docx(&bytes).map_err(|e| ParseError::ReadFailed(e.to_string()))?;

    fn paragraph_text(paragraph: &docx_rs::Paragraph) -> String {
        let mut out = String::new();
        for child in &paragraph.children {
            if let ParagraphChild::Run(run) = child {
                for run_child in &run.children {
                    if let RunChild::Text(t) = run_child {
                        out.push_str(&t.text);
                    }
                }
            }
        }
        out
    }

    let mut lines = Vec::new();
    for child in &docx.document.children {
        match child {
            DocumentChild::Paragraph(p) => lines.push(paragraph_text(p)),
            DocumentChild::Table(table) => {
                for TableChild::TableRow(row) in &table.rows {
                    for TableRowChild::TableCell(cell) in &row.cells {
                        for content in &cell.children {
                            if let TableCellContent::Paragraph(p) = content {
                                lines.push(paragraph_text(p));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    Ok(lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_supported_extensions_case_insensitively() {
        assert!(is_supported(Path::new("a/b/notes.MD")));
        assert!(is_supported(Path::new("report.PDF")));
        assert!(!is_supported(Path::new("photo.png")));
        assert!(!is_supported(Path::new("no-extension")));
    }

    #[test]
    fn words_split_across_pdf_lines_are_put_back_together() {
        // Taken from the user's Civil Code import.
        let joined = rejoin_hyphenated_words("arrecadação e liqui- dação da massa");
        assert_eq!(joined, "arrecadação e liquidação da massa");
        assert_eq!(
            rejoin_hyphenated_words("simplificado ao empre-\nsário rural"),
            "simplificado ao empresário rural"
        );
    }

    #[test]
    fn real_hyphens_survive() {
        for text in [
            "a inscrição far-se-á mediante requerimento",
            "um guarda-chuva novo",
            // A dash between words, not a broken word.
            "empresário - pessoa física",
            // Uppercase after the break is a new sentence, not a continuation.
            "termina aqui- Outra frase",
        ] {
            assert_eq!(rejoin_hyphenated_words(text), text, "mudou: {text}");
        }
    }

    #[test]
    fn a_text_file_with_only_whitespace_reports_no_text() {
        let path = std::env::temp_dir().join(format!("localmind-blank-{}.txt", std::process::id()));
        std::fs::write(&path, "   \n\n  ").unwrap();

        let err = extract_text(&path).unwrap_err();

        assert!(matches!(err, ParseError::NoTextFound));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn reads_markdown_as_plain_text() {
        let path = std::env::temp_dir().join(format!("localmind-doc-{}.md", std::process::id()));
        std::fs::write(&path, "# Título\n\nConteúdo do documento.").unwrap();

        let text = extract_text(&path).unwrap();

        assert!(text.contains("Conteúdo do documento."));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn unsupported_extension_is_rejected_by_name() {
        let path = std::env::temp_dir().join("whatever.png");
        assert!(matches!(
            extract_text(&path),
            Err(ParseError::UnsupportedFormat(ext)) if ext == "png"
        ));
    }
}

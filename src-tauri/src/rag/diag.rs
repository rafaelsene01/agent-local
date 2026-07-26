//! TEMPORARY diagnostic — delete after the investigation.
//! Run with: cargo test diag -- --ignored --nocapture

const PDF: &str = r"D:\aaaaaaaaaaa\documents\Código Civil 2 ed.pdf";
const VECTORS: &str = r"D:\aaaaaaaaaaa\vectors";
const NEEDLE: &str = "967";

/// What does the parser actually hand to the chunker?
#[test]
#[ignore]
fn dump_extracted_pdf_around_the_article() {
    let text = super::parsing::extract_text(std::path::Path::new(PDF)).expect("parse failed");
    println!("total chars: {}", text.len());

    let chars: Vec<char> = text.chars().collect();
    let mut found = 0;
    let hay: String = chars.iter().collect();
    for (byte_pos, _) in hay.match_indices(NEEDLE) {
        let start = hay[..byte_pos]
            .char_indices()
            .rev()
            .nth(120)
            .map(|(i, _)| i)
            .unwrap_or(0);
        let end = hay[byte_pos..]
            .char_indices()
            .nth(700)
            .map(|(i, _)| byte_pos + i)
            .unwrap_or(hay.len());
        println!("\n--- ocorrência @{byte_pos} ---\n{}", &hay[start..end]);
        found += 1;
        if found >= 4 {
            break;
        }
    }
    println!("\nocorrências de {NEEDLE}: {found}");
}

/// What is actually stored in LanceDB, verbatim?
#[tokio::test]
#[ignore]
async fn dump_stored_chunks_containing_the_article() {
    use arrow_array::{Array, Int32Array, StringArray};
    use futures_util::TryStreamExt;
    use lancedb::query::{ExecutableQuery, QueryBase};

    let db = lancedb::connect(VECTORS).execute().await.expect("connect");
    let table = db.open_table("chunks").execute().await.expect("open");
    println!("linhas na tabela: {}", table.count_rows(None).await.unwrap());

    let batches = table
        .query()
        .only_if("namespace = 'global'")
        .limit(100_000)
        .execute()
        .await
        .expect("scan")
        .try_collect::<Vec<_>>()
        .await
        .expect("collect");

    let mut hits = 0;
    let mut total = 0;
    for batch in batches {
        let texts = batch
            .column_by_name("text")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        let idx = batch
            .column_by_name("chunk_index")
            .unwrap()
            .as_any()
            .downcast_ref::<Int32Array>()
            .unwrap();
        for row in 0..batch.num_rows() {
            total += 1;
            let text = texts.value(row);
            if text.contains(NEEDLE) && hits < 3 {
                hits += 1;
                println!(
                    "\n=== chunk_index {} ({} chars) ===\n{}",
                    idx.value(row),
                    text.len(),
                    text
                );
            }
        }
    }
    println!("\nchunks no namespace global: {total}, com \"{NEEDLE}\": {hits}");
}

/// "ue" is not a Portuguese word — every one of them is a "que" that lost its
/// `q`. Counting them against the surviving "que" measures the damage.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| w.to_lowercase())
        .collect()
}

#[tokio::test]
#[ignore]
async fn how_much_of_the_corpus_is_corrupted() {
    use arrow_array::{Array, StringArray};
    use futures_util::TryStreamExt;
    use lancedb::query::{ExecutableQuery, QueryBase};

    let db = lancedb::connect(VECTORS).execute().await.unwrap();
    let table = db.open_table("chunks").execute().await.unwrap();
    let batches = table
        .query()
        .only_if("namespace = 'global'")
        .limit(100_000)
        .execute()
        .await
        .unwrap()
        .try_collect::<Vec<_>>()
        .await
        .unwrap();

    let (mut chunks, mut damaged) = (0usize, 0usize);
    let (mut ue, mut que) = (0usize, 0usize);
    let (mut letters, mut accented) = (0usize, 0usize);
    for batch in batches {
        let texts = batch
            .column_by_name("text")
            .unwrap()
            .as_any()
            .downcast_ref::<StringArray>()
            .unwrap();
        for row in 0..batch.num_rows() {
            let text = texts.value(row);
            chunks += 1;
            let mut hit = false;
            for word in words(text) {
                match word.as_str() {
                    "ue" => {
                        ue += 1;
                        hit = true;
                    }
                    "que" => que += 1,
                    _ => {}
                }
            }
            for c in text.chars().filter(|c| c.is_alphabetic()) {
                letters += 1;
                if !c.is_ascii() {
                    accented += 1;
                }
            }
            if hit {
                damaged += 1;
            }
        }
    }
    let pct = |a: usize, b: usize| if b == 0 { 0.0 } else { a as f64 * 100.0 / b as f64 };
    println!("chunks: {chunks}, com pelo menos um \"ue\" quebrado: {damaged} ({:.1}%)", pct(damaged, chunks));
    println!("\"que\" intactos: {que} | \"que\" sem o q: {ue} ({:.1}% perdidos)", pct(ue, ue + que));
    println!("letras acentuadas: {accented} de {letters} ({:.2}%) — PT normal fica ~4-5%", pct(accented, letters));
}

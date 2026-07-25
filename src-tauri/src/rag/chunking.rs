/// A slice of a document, in the order it appeared.
#[derive(Debug, Clone, PartialEq)]
pub struct TextChunk {
    pub index: usize,
    pub text: String,
}

/// Rough tokens-per-word factor. Real tokenization happens inside the
/// embedding model; counting words here only has to be close enough to keep
/// chunks under the model's limit, and it costs nothing.
const WORDS_PER_TOKEN: f32 = 0.75;

fn max_words(max_tokens: usize) -> usize {
    ((max_tokens as f32) * WORDS_PER_TOKEN).floor().max(1.0) as usize
}

/// Splits text into overlapping chunks on word boundaries. The overlap exists
/// so a sentence cut in half still appears whole in one of the neighbours —
/// without it, a fact spanning a boundary becomes unretrievable.
pub fn chunk_text(text: &str, max_tokens: usize, overlap: usize) -> Vec<TextChunk> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }

    let window = max_words(max_tokens);
    let overlap_words = max_words(overlap).min(window.saturating_sub(1));
    let step = window - overlap_words;

    let mut chunks = Vec::new();
    let mut start = 0;
    while start < words.len() {
        let end = (start + window).min(words.len());
        chunks.push(TextChunk {
            index: chunks.len(),
            text: words[start..end].join(" "),
        });
        if end == words.len() {
            break;
        }
        start += step;
    }
    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(n: usize) -> String {
        (0..n).map(|i| format!("w{i}")).collect::<Vec<_>>().join(" ")
    }

    #[test]
    fn empty_text_produces_no_chunks() {
        assert!(chunk_text("", 100, 10).is_empty());
        assert!(chunk_text("   \n\t ", 100, 10).is_empty());
    }

    #[test]
    fn text_shorter_than_the_window_is_a_single_chunk() {
        let chunks = chunk_text("uma frase curta", 100, 10);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].text, "uma frase curta");
        assert_eq!(chunks[0].index, 0);
    }

    #[test]
    fn longer_text_is_split_with_overlapping_words() {
        // window = 75 words, overlap = 15 words, so each chunk starts 60 later.
        let chunks = chunk_text(&words(200), 100, 20);
        assert!(chunks.len() > 1);

        let first: Vec<&str> = chunks[0].text.split(' ').collect();
        let second: Vec<&str> = chunks[1].text.split(' ').collect();
        let tail = &first[first.len() - 15..];
        assert_eq!(&second[..15], tail, "chunks must share their boundary words");
    }

    #[test]
    fn chunks_are_indexed_in_order_and_cover_the_whole_text() {
        let chunks = chunk_text(&words(200), 100, 20);
        for (i, chunk) in chunks.iter().enumerate() {
            assert_eq!(chunk.index, i);
        }
        assert!(
            chunks.last().unwrap().text.ends_with("w199"),
            "the last word must survive chunking"
        );
    }

    #[test]
    fn overlap_larger_than_the_window_still_advances() {
        // A pathological config must not loop forever or drop everything.
        let chunks = chunk_text(&words(50), 10, 1000);
        assert!(!chunks.is_empty());
        assert!(chunks.last().unwrap().text.ends_with("w49"));
    }
}

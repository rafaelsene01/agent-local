// SPEC: chat-messaging (CHAT-08, CHAT-09, CHAT-13, CHAT-15), documents-rag (DOC-12),
//       conversation-memory (MEM-04, MEM-05, MEM-06, MEM-08, MEM-10, MEM-11, MEM-12, MEM-13)

use crate::chat::memory;
use crate::db::DbState;
use crate::providers::ChatMessage;
use crate::rag::store::{chat_namespace, VectorStore, GLOBAL_NAMESPACE};
use crate::rag::{embedding, onnxruntime, pipeline};
use rusqlite::params;
use tauri::{AppHandle, Manager};

/// Rough characters-per-token used for budgeting. Exact tokenization varies
/// per model; being slightly conservative here costs a little context and
/// avoids overflowing the window, which would fail the whole request.
const CHARS_PER_TOKEN: usize = 4;

/// Used when no context length was configured for the active model.
const DEFAULT_CONTEXT_TOKENS: u32 = 4096;

const TOP_K: usize = 4;
const RECENT_HISTORY_LIMIT: usize = 20;

/// The conversation memory gets its own cap instead of sharing `TOP_K` with the
/// documents (MEM-12). A shared cap would let a long conversation win slots from
/// the file the user explicitly imported, which is the opposite of the priority
/// the layers were designed with.
///
/// **One, not two, because the relevance floor does not work on this layer.**
/// Measured against the real embedding model (`chat::memory::memory_quality`):
/// the right turn does win — 0.2484 against 0.3413 for an unrelated one — but a
/// question about something the conversation never discussed still leaves
/// *every* stored turn under `RELATIVE_DISTANCE_FLOOR`. The floor separates
/// documents (AD-025 measured 3.9×) because a real passage hit lands near 0.09;
/// conversation turns all sit in the 0.25–0.38 band, so the ratio to the best
/// never reaches 3×.
///
/// With no working filter, the cap *is* the filter. One irrelevant turn next to
/// the question is the same failure mode AD-033 measured, and two is twice as
/// much of it. Raising this again needs an absolute ceiling that has been
/// measured against a real conversation, not against the three synthetic turns
/// that produced these numbers.
const MEMORY_TOP_K: usize = 1;

/// How many turns the memory search fetches before anything is discarded.
///
/// It has to exceed `MEMORY_TOP_K` for the same reason `PER_NAMESPACE_K` exceeds
/// `TOP_K`: the pool is filtered after it is built, so a pool the size of the
/// final cap can be emptied by a single rejection.
///
/// Measured, not assumed. Asking for exactly one candidate returned, verbatim
/// from the running app, the *user's own earlier phrasing of the same question*
/// — already quoted in the recent history, therefore dropped by the duplicate
/// filter, therefore nothing recalled at all. A question is nearest to itself,
/// so the more naturally the user re-asks something, the more reliably the old
/// funnel returned nothing.
const MEMORY_CANDIDATES: usize = 8;

/// Percentage of the remaining budget withheld from the recent history so the
/// memory has something to spend.
///
/// Without it the memory only ever got the history's leftovers, and a
/// conversation long enough to need memory is precisely one whose history fills
/// the budget by itself. Measured in the app, not reasoned about: 9 turns and
/// 21 993 characters of history against an 8192-character budget left
/// `recall_blocks` with exactly 0 to spend, and the model answered "não tenho a
/// capacidade de acessar informações" to a question about its own first turn —
/// including when the question used the planted word verbatim. The layer was
/// unreachable in the only case it exists for.
///
/// A reservation rather than a reordering: AD-033 measured that the model
/// answers from what sits next to the question, so the recent turns keep both
/// their priority and their position. They give up at most this share, and get
/// back whatever the memory does not use.
const MEMORY_BUDGET_SHARE: usize = 15;

/// How many candidates each namespace contributes before ranking. The winners
/// are picked across namespaces, so every namespace has to offer more than the
/// final count for the ranking to have anything to choose from.
const PER_NAMESPACE_K: usize = TOP_K;

/// Relevance floor, expressed relative to the best hit of the same query.
///
/// An absolute floor does not separate anything with this embedding model:
/// measured on the real corpus (AD-025), an unrelated passage still scores
/// 0.826 cosine against a paraphrase's 0.957 — squared-L2 distances of ~0.35
/// and ~0.09. The ratio to the best hit does separate them.
const RELATIVE_DISTANCE_FLOOR: f32 = 3.0;

/// Keeps an exact match (distance 0) from making every other passage look
/// irrelevant, since anything times zero is zero.
const MIN_DISTANCE_CUTOFF: f32 = 0.1;

/// The "don't offer more help" clauses are aimed at Phi-3.5's habit of closing
/// every answer with a courtesy paragraph ("sinta-se à vontade para
/// perguntar"). It reduces the filler; it doesn't eliminate it, because the
/// tendency is the model's own.
///
/// Length is tied to the request rather than fixed at "as few sentences as
/// possible": that wording fought every "continue this text" and "transcribe
/// this passage", which is exactly what a document base gets asked for.
const SYSTEM_PROMPT: &str = "Você é um assistente local e privado. Responda apenas o que foi \
perguntado. Não repita a pergunta, não ofereça ajuda adicional, não comente a própria resposta \
e não escreva parágrafos de cortesia. Ajuste o tamanho da resposta ao pedido: seja breve numa \
pergunta objetiva e completo quando pedirem para continuar, transcrever ou detalhar um texto. \
Diga claramente quando não souber.";

/// The citation instruction rides with the context, not with the base prompt:
/// a small model told to cite sources when none were given invents them —
/// observed live, answering with "[fonte: GPT-3 informações geral]".
const CONTEXT_PREAMBLE: &str = "Trechos dos documentos do usuário. Baseie a resposta neles e \
cite o nome do arquivo indicado em [fonte: ...] ao usar um trecho. Se a resposta não estiver \
aqui, diga isso.";

/// Recalled turns get their own preamble because the document one orders the
/// model to cite a filename, and a past exchange has none. Told to cite a source
/// that does not exist, this model invents one — observed live as
/// "[fonte: GPT-3 informações geral]" (MEM-06).
const MEMORY_PREAMBLE: &str = "Trechos de mensagens anteriores desta mesma conversa, só para \
lembrar o que já foi dito. Não são documentos: não os cite como fonte e não os repita se não \
forem necessários.";

/// Every retrieved chunk carries the file it came from (DOC-12): without it
/// the model has nothing to cite and the user can't check the answer.
fn source_block(label: &str, text: &str) -> String {
    format!("[fonte: {label}]\n{text}")
}

/// The marker deliberately does not use the `[fonte: ...]` shape, so a model
/// that copies the label into its answer produces something the user reads as
/// "an earlier message", not as a file that does not exist.
fn memory_block(text: &str) -> String {
    format!("[conversa anterior]\n{text}")
}

/// The room reserved for the answer is the same number the provider is told to
/// generate. A fixed 512 here while `answer_token_budget` asked for up to 2048
/// meant the prompt could be built right up to a limit the answer would then
/// blow past — harmless on a 21760-token window, an overflow on a hand-set
/// 4096 one.
fn budget_chars(context_length: Option<u32>) -> usize {
    let total = context_length.unwrap_or(DEFAULT_CONTEXT_TOKENS);
    let usable = total.saturating_sub(crate::providers::answer_token_budget(context_length));
    usable as usize * CHARS_PER_TOKEN
}

struct Budget {
    remaining: usize,
}

impl Budget {
    /// Takes what fits and truncates the rest, instead of dropping a whole
    /// category — a partially included document still answers questions
    /// (CHAT-15).
    fn take(&mut self, text: &str) -> Option<String> {
        if self.remaining == 0 {
            return None;
        }
        if text.len() <= self.remaining {
            self.remaining -= text.len();
            return Some(text.to_string());
        }
        let mut cut = self.remaining;
        while cut > 0 && !text.is_char_boundary(cut) {
            cut -= 1;
        }
        self.remaining = 0;
        if cut == 0 {
            None
        } else {
            Some(text[..cut].to_string())
        }
    }
}

/// `exclude_id` is the message being answered: `send_message` persists it
/// before assembling, so without this it would come back from the database and
/// be sent again right before itself. Two identical user turns in a row make
/// the model ramble instead of answering — verified against the sidecar, where
/// the duplicated prompt never produced a stop token and the single one did.
/// A history message together with its id.
///
/// The id is what the memory layer deduplicates against: a turn that is already
/// in the prompt verbatim must not also arrive as a recalled passage (MEM-05).
#[derive(Debug, Clone)]
struct HistoryEntry {
    id: String,
    message: ChatMessage,
}

fn recent_history(
    app: &AppHandle,
    chat_id: &str,
    exclude_id: &str,
) -> Result<Vec<HistoryEntry>, String> {
    let db = app.state::<DbState>();
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = crate::db::require_conn(&guard)?;
    let mut stmt = sql
        .prepare(
            "SELECT id, role, content FROM messages WHERE chat_id = ?1 AND id <> ?2
             ORDER BY created_at DESC LIMIT ?3",
        )
        .map_err(|e| e.to_string())?;
    let mut rows: Vec<HistoryEntry> = stmt
        .query_map(params![chat_id, exclude_id, RECENT_HISTORY_LIMIT as i64], |row| {
            Ok(HistoryEntry {
                id: row.get(0)?,
                message: ChatMessage {
                    role: row.get(1)?,
                    content: row.get(2)?,
                },
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<_, _>>()
        .map_err(|e| e.to_string())?;
    // Queried newest-first for the LIMIT, replayed oldest-first for the model.
    rows.reverse();
    Ok(rows)
}

/// Whole small attachments bypass retrieval entirely (CHAT-09): chunking a
/// three-line note only loses information.
fn injected_attachments(app: &AppHandle, chat_id: &str) -> Result<Vec<(String, String)>, String> {
    let db = app.state::<DbState>();
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = crate::db::require_conn(&guard)?;
    let mut stmt = sql
        .prepare(
            "SELECT filename, extracted_text FROM chat_attachments
             WHERE chat_id = ?1 AND status = 'injected_whole' AND extracted_text IS NOT NULL",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![chat_id], |row| Ok((row.get(0)?, row.get(1)?)))
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// Keeps the newest turns and drops the oldest when the budget runs out.
///
/// The naive direction — walking the history oldest-first and taking what fits
/// — spends the budget on turns the model no longer needs and starves the
/// exchange the question actually follows from.
/// Fits the history while holding `MEMORY_BUDGET_SHARE` back for the memory,
/// then returns the unspent remainder to the budget.
///
/// The two steps live together because the guarantee is in their pairing: the
/// withheld slice has to be restored whether or not the history exhausted its
/// own share, or a short conversation would silently lose the reservation.
fn fit_history_reserving_memory(
    history: Vec<HistoryEntry>,
    budget: &mut Budget,
    use_memory: bool,
) -> Vec<HistoryEntry> {
    // A chat with the toggle off must build exactly the prompt it built before
    // this reservation existed (MEM-19).
    let reserved = if use_memory {
        budget.remaining * MEMORY_BUDGET_SHARE / 100
    } else {
        0
    };
    budget.remaining -= reserved;
    let kept = fit_history(history, budget);
    budget.remaining += reserved;
    kept
}

fn fit_history(history: Vec<HistoryEntry>, budget: &mut Budget) -> Vec<HistoryEntry> {
    let mut kept: Vec<HistoryEntry> = Vec::new();
    for entry in history.into_iter().rev() {
        let Some(content) = budget.take(&entry.message.content) else {
            break;
        };
        kept.push(HistoryEntry {
            id: entry.id,
            message: ChatMessage {
                role: entry.message.role,
                content,
            },
        });
    }
    kept.reverse();
    kept
}

/// Puts the retrieved passages in the same turn as the question, right above
/// it, instead of in a system block at the top of the prompt.
///
/// A small model answers from what sits next to the question. With the document
/// thousands of tokens away — behind the whole history — Phi-3.5 reproduced its
/// own earlier answers verbatim instead of the source: three wrong
/// continuations of the same article, the last one a copy of the first.
/// The two groups are separate sections with separate preambles, and the
/// documents sit closer to the question than the recalled turns: the document is
/// what the user explicitly imported, the memory is supporting context. Proximity
/// to the question is what the model actually reads, so the ordering is a
/// priority statement, not formatting.
fn question_with_context(
    new_message: &str,
    memory_blocks: &[String],
    context_blocks: &[String],
    memory_outranks_documents: bool,
) -> String {
    let memory_section = (!memory_blocks.is_empty()).then(|| {
        format!(
            "{MEMORY_PREAMBLE}\n\n{}",
            memory_blocks.join("\n\n---\n\n")
        )
    });
    let document_section = (!context_blocks.is_empty()).then(|| {
        format!(
            "{CONTEXT_PREAMBLE}\n\n{}",
            context_blocks.join("\n\n---\n\n")
        )
    });

    // Nearest last, because last is what touches the question.
    //
    // Documents used to be unconditionally closer, on the reasoning that an
    // imported file is the user's explicit intent. That held right up until the
    // base had nothing to do with the question: a Civil Code passage at 0.3150
    // sat against the question while the turn holding the answer, at 0.2817, sat
    // behind it — and the model answered from the Civil Code (AD-050, measured
    // through the app's own UI). Priority by layer is a guess about relevance;
    // the distance is a measurement of it.
    let mut sections: Vec<String> = Vec::new();
    if memory_outranks_documents {
        sections.extend(document_section);
        sections.extend(memory_section);
    } else {
        sections.extend(memory_section);
        sections.extend(document_section);
    }

    if sections.is_empty() {
        return new_message.to_string();
    }
    sections.push(new_message.to_string());
    sections.join("\n\n---\n\n")
}

/// The prompt plus what went wrong building it.
///
/// `retrieval_error` exists because a broken vector store used to be a single
/// `eprintln!`: the answer came back with no documents in it and the user had
/// no way to tell "the knowledge base failed" from "the model ignored my
/// document" — which is literally the question that opened the AD-033
/// investigation.
pub struct Assembled {
    pub messages: Vec<ChatMessage>,
    pub retrieval_error: Option<String>,
}

/// Builds the final message list: system prompt, recent history, then the
/// question with the retrieved passages attached to it. The question is never
/// truncated — everything else is.
pub async fn assemble(
    app: &AppHandle,
    chat_id: &str,
    new_message: &str,
    new_message_id: &str,
    use_global_rag: bool,
    use_memory: bool,
    context_length: Option<u32>,
) -> Result<Assembled, String> {
    let mut budget = Budget {
        remaining: budget_chars(context_length).saturating_sub(new_message.len()),
    };

    let mut context_blocks: Vec<String> = Vec::new();

    for (filename, text) in injected_attachments(app, chat_id)? {
        if let Some(part) = budget.take(&source_block(&filename, &text)) {
            context_blocks.push(part);
        }
    }

    // Retrieval only runs when something was indexed; embedding the question
    // otherwise would download a model for nothing.
    let chat_ns = chat_namespace(chat_id);
    let namespaces: Vec<&str> = if use_global_rag {
        vec![chat_ns.as_str(), GLOBAL_NAMESPACE]
    } else {
        vec![chat_ns.as_str()]
    };

    let mut retrieval_error = None;
    let mut recalled: Vec<MemoryHit> = Vec::new();
    let mut memory_outranks_documents = false;
    if budget.remaining > 0 {
        let memory_for = if use_memory { Some(chat_id) } else { None };
        match retrieve(app, &namespaces, memory_for, new_message).await {
            Ok(found) => {
                for chunk in found.documents {
                    if let Some(part) = budget.take(&chunk) {
                        context_blocks.push(part);
                    }
                }
                recalled = found.memory;
                memory_outranks_documents = found.memory_outranks_documents;
                // The answer still goes out with its documents; the user is
                // told the memory did not take part (MEM-13).
                if let Some(e) = found.memory_error {
                    eprintln!("conversation memory not retrieved: {e}");
                    retrieval_error = Some(e);
                }
            }
            // Retrieval is an enhancement: a broken vector store or a missing
            // embedding model must not block the conversation — but it is
            // reported, not swallowed.
            Err(e) => {
                eprintln!("retrieval skipped: {e}");
                retrieval_error = Some(e);
            }
        }
    }

    let mut messages = vec![ChatMessage::system(SYSTEM_PROMPT)];
    let history = recent_history(app, chat_id, new_message_id)?;
    let kept_history = fit_history_reserving_memory(history, &mut budget, use_memory);

    // The memory spends what is left after the documents and the recent turns
    // (MEM-10), plus the slice withheld above so that "what is left" is never
    // zero. The order is still the whole point: AD-033 measured that the turns
    // sitting next to the question are what the model answers from, so the new
    // layer never pushes them out — it only stops being crowded out itself.
    //
    // The exclusion set is what *survived* the budget, not what was queried —
    // a turn the budget dropped is no longer in the prompt, so recalling it is
    // exactly right rather than a duplicate.
    let verbatim: std::collections::HashSet<&str> =
        kept_history.iter().map(|entry| entry.id.as_str()).collect();
    let memory_blocks = recall_blocks(recalled, &verbatim, &mut budget);

    messages.extend(kept_history.into_iter().map(|entry| entry.message));
    messages.push(ChatMessage::user(question_with_context(
        new_message,
        &memory_blocks,
        &context_blocks,
        memory_outranks_documents,
    )));
    Ok(Assembled {
        messages: merge_consecutive_turns(messages),
        retrieval_error,
    })
}

/// Turns the recalled hits into prompt blocks, dropping the ones already in the
/// prompt verbatim and stopping when the budget runs out.
///
/// Whatever is left of the budget at this point is all the memory gets: it is
/// the last layer to be served (MEM-10), and running out is a normal outcome
/// rather than a failure (MEM-11).
fn recall_blocks(
    recalled: Vec<MemoryHit>,
    verbatim: &std::collections::HashSet<&str>,
    budget: &mut Budget,
) -> Vec<String> {
    recalled
        .into_iter()
        .filter(|hit| !verbatim.contains(hit.answer_id.as_str()))
        // The cap applies to what survives the filter, so a pool full of turns
        // already in the prompt costs relevance rather than the whole layer.
        .take(MEMORY_TOP_K)
        .filter_map(|hit| budget.take(&memory_block(&hit.text)))
        .collect()
}

/// Chat templates assume the roles alternate. A generation that was cancelled
/// or failed leaves the question persisted with no answer after it, so every
/// later request would carry two `user` turns in a row.
///
/// Measured against the running sidecar, that single malformation flips
/// Phi-3.5 from answering to acknowledging: "Entendido! Se você tiver mais
/// perguntas… fique à vontade" instead of "Olá! Como posso ajudá-lo hoje?".
/// Merging keeps every word the user wrote while restoring the alternation.
fn merge_consecutive_turns(messages: Vec<ChatMessage>) -> Vec<ChatMessage> {
    let mut merged: Vec<ChatMessage> = Vec::with_capacity(messages.len());
    for message in messages {
        match merged.last_mut() {
            Some(previous) if previous.role == message.role => {
                previous.content.push_str("\n\n");
                previous.content.push_str(&message.content);
            }
            _ => merged.push(message),
        }
    }
    merged
}

/// A hit plus the namespace it came from, which the store does not carry back.
#[derive(Debug, Clone)]
struct Candidate {
    namespace: String,
    doc_id: String,
    chunk_index: i32,
    text: String,
    distance: f32,
}

impl Candidate {
    fn key(&self) -> (String, String, i32) {
        (self.namespace.clone(), self.doc_id.clone(), self.chunk_index)
    }
}

/// Ranks every candidate together and keeps the best `top_k`.
///
/// Taking `top_k` per namespace instead had two effects, both invisible until
/// measured: the chat's own attachment always came before the document base no
/// matter how weak the match, because namespace order decided it; and when the
/// budget ran short the later namespace was the one truncated away.
///
/// Chunks with no score (NaN — see `rows_from_batch`) sort last and are never
/// used to compute the cutoff, so a missing `_distance` degrades to the old
/// behavior instead of dropping everything.
fn rank_candidates(mut candidates: Vec<Candidate>, top_k: usize) -> Vec<Candidate> {
    candidates.sort_by(|a, b| a.distance.total_cmp(&b.distance));

    if let Some(best) = candidates.iter().map(|c| c.distance).find(|d| !d.is_nan()) {
        let cutoff = (best * RELATIVE_DISTANCE_FLOOR).max(MIN_DISTANCE_CUTOFF);
        candidates.retain(|c| c.distance.is_nan() || c.distance <= cutoff);
    }

    candidates.truncate(top_k);
    candidates
}

/// A recalled exchange, still carrying the id of the answer it came from so the
/// caller can drop it if that turn is already in the prompt verbatim (MEM-05).
struct MemoryHit {
    answer_id: String,
    text: String,
}

/// What one round of retrieval produced. The two groups stay apart all the way
/// to the prompt: they have different preambles, different caps and different
/// positions relative to the question.
struct Retrieved {
    documents: Vec<String>,
    memory: Vec<MemoryHit>,
    /// Reported separately instead of failing the whole retrieval: the memory
    /// is the last layer and the least important one, and letting its failure
    /// discard passages that were already found would drop the user's own
    /// documents out of the prompt — the exact failure mode AD-033 chased.
    memory_error: Option<String>,
    /// The best recalled turn is nearer the question than the best document.
    ///
    /// This is what decides which block ends up adjacent to the question, and
    /// it is the whole fix for AD-050: AD-033 measured that the model answers
    /// from what sits next to the question, so the position has to follow the
    /// ranking rather than the layer.
    memory_outranks_documents: bool,
}

/// Appends the chunk that follows a hit, when it was not already selected on
/// its own merit — the passage that continues a hit is the next chunk, not the
/// next closest one.
async fn with_neighbour(
    store: &VectorStore,
    candidate: &Candidate,
    selected: &std::collections::HashSet<(String, String, i32)>,
) -> String {
    let mut text = candidate.text.clone();
    let neighbor_key = (
        candidate.namespace.clone(),
        candidate.doc_id.clone(),
        candidate.chunk_index + 1,
    );
    if !selected.contains(&neighbor_key) {
        if let Ok(Some(next)) = store
            .chunk_at(&candidate.namespace, &candidate.doc_id, candidate.chunk_index + 1)
            .await
        {
            text.push('\n');
            text.push_str(&next.text);
        }
    }
    text
}

/// `memory_for` is `Some(chat_id)` only when that conversation has memory
/// enabled. Passing the chat id rather than a namespace is deliberate: the
/// namespace is built inside `chat::memory`, so no caller can hand this
/// function another conversation's memory (MEM-08).
async fn retrieve(
    app: &AppHandle,
    namespaces: &[&str],
    memory_for: Option<&str>,
    question: &str,
) -> Result<Retrieved, String> {
    let store = VectorStore::open(&pipeline::vectors_dir(app)?)
        .await
        .map_err(|e| e.to_string())?;

    onnxruntime::ensure_dylib(app).await?;
    let owned_question = question.to_string();
    // Embedded once and reused: both layers answer the same question, and this
    // is CPU-bound work with the model held in memory.
    let query_vec =
        tauri::async_runtime::spawn_blocking(move || embedding::embed_query(&owned_question))
            .await
            .map_err(|e| e.to_string())?
            .map_err(|e| e.to_string())?;

    let mut candidates = Vec::new();
    for namespace in namespaces {
        let found = store
            .search(namespace, &query_vec, PER_NAMESPACE_K)
            .await
            .map_err(|e| e.to_string())?;
        for chunk in found {
            candidates.push(Candidate {
                namespace: namespace.to_string(),
                doc_id: chunk.doc_id,
                chunk_index: chunk.chunk_index,
                text: chunk.text,
                distance: chunk.distance,
            });
        }
    }

    let mut memory_error = None;
    let memory_namespace = memory_for.map(memory::memory_namespace);
    let mut memory_candidates: Vec<Candidate> = Vec::new();
    if let (Some(chat_id), Some(namespace)) = (memory_for, memory_namespace.as_ref()) {
        let found = match memory::search(&store, chat_id, &query_vec, MEMORY_CANDIDATES).await {
            Ok(found) => found,
            Err(e) => {
                memory_error = Some(e);
                Vec::new()
            }
        };
        memory_candidates = found
            .into_iter()
            .map(|chunk| Candidate {
                namespace: namespace.clone(),
                doc_id: chunk.doc_id,
                chunk_index: chunk.chunk_index,
                text: chunk.text,
                distance: chunk.distance,
            })
            .collect();
    }

    // One pool, one ranking (AD-050).
    //
    // The two layers answer the *same* question with the *same* metric, so
    // their distances are directly comparable — and ranking them apart is what
    // let an unrelated document take the four slots nearest the question while
    // the turn that actually held the answer sat further away with one slot.
    // Measured against the user's own base: the best Civil Code chunk scored
    // 0.3150 and the recalled turn 0.2817, and the model answered from the Civil
    // Code.
    //
    // The floor stays relative because an absolute one is not available: the
    // worst genuine hit on that corpus is 0.3077 and the best irrelevant one
    // 0.3150, a 0.0073 gap that no threshold survives (`retrieval_quality`).
    // What separates them here is not a cutoff, it is the comparison.
    let mut pool = candidates;
    pool.extend(memory_candidates.iter().cloned());
    let ranked_all = rank_candidates(pool, usize::MAX);

    let is_memory = |c: &Candidate| Some(&c.namespace) == memory_namespace.as_ref();
    let memory_best = ranked_all.iter().find(|c| is_memory(c)).map(|c| c.distance);
    let documents_best = ranked_all.iter().find(|c| !is_memory(c)).map(|c| c.distance);

    // The cap is now shared. A conversation with something to recall costs the
    // documents one slot — not four, and not zero: `MEMORY_TOP_K` still bounds
    // how much of the prompt a long conversation can take from the file the
    // user just imported, which is what MEM-12 was for.
    let document_slots = if memory_best.is_some() {
        TOP_K.saturating_sub(MEMORY_TOP_K)
    } else {
        TOP_K
    };

    let ranked_documents: Vec<Candidate> = ranked_all
        .iter()
        .filter(|c| !is_memory(c))
        .take(document_slots)
        .cloned()
        .collect();
    // Every surviving turn is carried forward, not just `MEMORY_TOP_K` of them:
    // the duplicate filter runs later, and cutting to the final cap before it is
    // exactly the funnel AD-047 had to undo.
    let ranked_memory: Vec<Candidate> = ranked_all
        .into_iter()
        .filter(|c| is_memory(c))
        .collect();

    let selected: std::collections::HashSet<_> = ranked_documents.iter().map(|c| c.key()).collect();
    let mut documents = Vec::new();
    for candidate in &ranked_documents {
        let text = with_neighbour(&store, candidate, &selected).await;
        let label = source_name(app, &candidate.namespace, &candidate.doc_id);
        documents.push(source_block(&label, &text));
    }

    let selected_memory: std::collections::HashSet<_> =
        ranked_memory.iter().map(|c| c.key()).collect();
    let mut memory = Vec::new();
    for candidate in &ranked_memory {
        memory.push(MemoryHit {
            answer_id: candidate.doc_id.clone(),
            text: with_neighbour(&store, candidate, &selected_memory).await,
        });
    }

    Ok(Retrieved {
        documents,
        memory,
        memory_error,
        memory_outranks_documents: nearer(memory_best, documents_best),
    })
}

/// Whether `a` is a nearer hit than `b`, with "no hit at all" losing to any hit.
///
/// Split out because the comparison decides which block sits next to the
/// question, and `Option<f32>` ordering is the kind of thing that is wrong in a
/// way nobody notices — `None` must not compare as zero distance.
fn nearer(a: Option<f32>, b: Option<f32>) -> bool {
    match (a, b) {
        (Some(a), Some(b)) => a < b,
        (Some(_), None) => true,
        _ => false,
    }
}

/// Resolves a `doc_id` back to the file the user recognizes. Global chunks
/// come from `documents`, chat chunks from `chat_attachments`; an id with no
/// row left (deleted mid-flight) falls back to a generic label instead of
/// failing the whole retrieval.
fn source_name(app: &AppHandle, namespace: &str, doc_id: &str) -> String {
    let fallback = || "documento".to_string();
    let db = app.state::<DbState>();
    let Ok(guard) = db.0.lock() else {
        return fallback();
    };
    let Some(sql) = guard.as_ref() else {
        return fallback();
    };
    let query = if namespace == GLOBAL_NAMESPACE {
        "SELECT filename FROM documents WHERE id = ?1"
    } else {
        "SELECT filename FROM chat_attachments WHERE id = ?1"
    };
    sql.query_row(query, params![doc_id], |row| row.get::<_, String>(0))
        .unwrap_or_else(|_| fallback())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(namespace: &str, chunk_index: i32, distance: f32) -> Candidate {
        Candidate {
            namespace: namespace.to_string(),
            doc_id: "doc".to_string(),
            chunk_index,
            text: format!("{namespace}#{chunk_index}"),
            distance,
        }
    }

    #[test]
    fn the_best_match_wins_regardless_of_which_namespace_it_came_from() {
        // The chat attachment used to come first simply because its namespace
        // was queried first, pushing a much closer document chunk out.
        let ranked = rank_candidates(
            vec![
                candidate("chat:1", 0, 0.30),
                candidate("global", 7, 0.09),
                candidate("chat:1", 1, 0.25),
            ],
            3,
        );

        assert_eq!(ranked[0].namespace, "global");
        assert!(ranked.windows(2).all(|w| w[0].distance <= w[1].distance));
    }

    #[test]
    fn a_passage_far_worse_than_the_best_one_is_dropped() {
        // Real numbers from AD-025: a paraphrase lands around 0.09 and an
        // unrelated passage around 0.35 in squared-L2.
        let ranked = rank_candidates(
            vec![
                candidate("global", 0, 0.09),
                candidate("global", 1, 0.20),
                candidate("global", 2, 0.35),
            ],
            4,
        );

        let kept: Vec<i32> = ranked.iter().map(|c| c.chunk_index).collect();
        assert_eq!(kept, vec![0, 1]);
    }

    #[test]
    fn an_exact_match_does_not_disqualify_everything_else() {
        // distance * 3 is still zero, so without a minimum cutoff a verbatim
        // hit would throw away every passage around it.
        let ranked = rank_candidates(
            vec![candidate("global", 0, 0.0), candidate("global", 1, 0.08)],
            4,
        );
        assert_eq!(ranked.len(), 2);
    }

    #[test]
    fn unscored_chunks_are_kept_and_sorted_last() {
        let ranked = rank_candidates(
            vec![candidate("global", 1, f32::NAN), candidate("global", 0, 0.09)],
            4,
        );
        assert_eq!(ranked.len(), 2);
        assert_eq!(ranked[0].chunk_index, 0);
        assert!(ranked[1].distance.is_nan());
    }

    #[test]
    fn ranking_keeps_at_most_top_k() {
        let candidates = (0..10).map(|i| candidate("global", i, 0.09)).collect();
        assert_eq!(rank_candidates(candidates, TOP_K).len(), TOP_K);
    }

    #[test]
    fn the_prompt_budget_reserves_exactly_what_the_answer_is_allowed_to_use() {
        // A 4096 window asks for at most 2048 answer tokens, so the prompt gets
        // the other half — not 4096 - 512.
        assert_eq!(
            budget_chars(Some(4096)),
            2048 * CHARS_PER_TOKEN,
            "prompt budget must not overlap the answer budget"
        );
        assert_eq!(
            budget_chars(Some(21760)),
            (21760 - 2048) as usize * CHARS_PER_TOKEN
        );
    }

    #[test]
    fn budget_truncates_instead_of_dropping() {
        let mut budget = Budget { remaining: 10 };
        assert_eq!(budget.take("12345").unwrap(), "12345");
        assert_eq!(budget.take("abcdefghij").unwrap(), "abcde");
        assert!(budget.take("more").is_none());
    }

    #[test]
    fn truncation_never_splits_a_multibyte_character() {
        let mut budget = Budget { remaining: 3 };
        // 'ç' is two bytes, so cutting at byte 3 would land mid-character.
        let taken = budget.take("açaí").unwrap();
        assert!(taken.chars().all(|c| "açaí".contains(c)));
    }

    #[test]
    fn an_unanswered_question_does_not_leave_two_user_turns_in_a_row() {
        let merged = merge_consecutive_turns(vec![
            ChatMessage::system("regras"),
            ChatMessage::system("contexto"),
            // The generation for this one was cancelled, so no answer follows.
            ChatMessage::user("oi"),
            ChatMessage::user("o que é um banco de dados?"),
        ]);

        let roles: Vec<&str> = merged.iter().map(|m| m.role.as_str()).collect();
        assert_eq!(roles, vec!["system", "user"]);
        assert_eq!(merged[1].content, "oi\n\no que é um banco de dados?");
        assert!(merged[0].content.contains("regras") && merged[0].content.contains("contexto"));
    }

    #[test]
    fn a_healthy_conversation_is_left_exactly_as_it_is() {
        let original = vec![
            ChatMessage::system("regras"),
            ChatMessage::user("pergunta"),
            ChatMessage {
                role: "assistant".to_string(),
                content: "resposta".to_string(),
            },
            ChatMessage::user("outra pergunta"),
        ];

        let merged = merge_consecutive_turns(original.clone());

        assert_eq!(merged.len(), original.len());
        let roles: Vec<&str> = merged.iter().map(|m| m.role.as_str()).collect();
        assert_eq!(roles, vec!["system", "user", "assistant", "user"]);
    }

    fn turn(role: &str, content: &str) -> HistoryEntry {
        HistoryEntry {
            id: format!("id-{content}"),
            message: ChatMessage {
                role: role.to_string(),
                content: content.to_string(),
            },
        }
    }

    #[test]
    fn a_tight_budget_drops_the_oldest_turns_and_keeps_the_newest() {
        let history = vec![
            turn("user", "pergunta antiga"),
            turn("assistant", "resposta antiga"),
            turn("user", "pergunta recente"),
        ];
        // Room for the last turn only.
        let mut budget = Budget { remaining: 16 };

        let kept = fit_history(history, &mut budget);

        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].message.content, "pergunta recente");
    }

    /// The regression the UAT of 2026-07-27 found in the app: the nearest stored
    /// turn to a re-asked question is the earlier asking of it, which is already
    /// quoted verbatim and therefore dropped. When the cap was applied before
    /// the filter, that single rejection emptied the layer.
    #[test]
    fn a_recalled_turn_already_in_the_prompt_does_not_consume_the_cap() {
        let verbatim: std::collections::HashSet<&str> = ["repetida"].into_iter().collect();
        let mut budget = Budget { remaining: 10_000 };

        let blocks = recall_blocks(
            vec![
                hit("repetida", "Usuário: com que apelido eu batizei o trabalho?"),
                hit("plantada", "Usuário: guarde este dado\nAssistente: Pantera Cinzenta"),
            ],
            &verbatim,
            &mut budget,
        );

        assert_eq!(blocks.len(), 1, "o turno seguinte tem que ocupar a vaga");
        assert!(blocks[0].contains("Pantera Cinzenta"));
    }

    /// The cap is still a cap once the duplicates are gone.
    #[test]
    fn the_memory_never_injects_more_than_its_own_cap() {
        let verbatim = std::collections::HashSet::new();
        let mut budget = Budget { remaining: 10_000 };

        let blocks = recall_blocks(
            (0..5).map(|i| hit(&format!("t{i}"), "um turno")).collect(),
            &verbatim,
            &mut budget,
        );

        assert_eq!(blocks.len(), MEMORY_TOP_K);
    }

    /// The regression the UAT of 2026-07-27 exposed: with the history alone
    /// larger than the budget, `fit_history` spent every character and the
    /// memory was handed 0. In the app that showed up as the model denying it
    /// could remember its own first turn — the one case the feature exists for.
    #[test]
    fn a_history_bigger_than_the_budget_still_leaves_the_memory_something() {
        let history: Vec<HistoryEntry> = (0..20)
            .map(|i| turn("user", &format!("{i}{}", "x".repeat(1000))))
            .collect();
        let mut budget = Budget { remaining: 8192 };

        let kept = fit_history_reserving_memory(history, &mut budget, true);

        assert!(!kept.is_empty(), "the recent turns still come first");
        assert_eq!(
            budget.remaining,
            8192 * MEMORY_BUDGET_SHARE / 100,
            "the reserved slice has to survive a history that would eat everything"
        );
    }

    /// The reservation is not a tax on chats that do not use memory (MEM-19).
    #[test]
    fn with_memory_off_the_history_still_receives_the_whole_budget() {
        let history: Vec<HistoryEntry> = (0..20)
            .map(|i| turn("user", &format!("{i}{}", "x".repeat(1000))))
            .collect();
        let mut budget = Budget { remaining: 8192 };

        fit_history_reserving_memory(history, &mut budget, false);

        assert_eq!(budget.remaining, 0);
    }

    /// A short conversation must not lose the reservation: whatever the history
    /// leaves unspent goes back, so the memory is never worse off than before.
    #[test]
    fn an_unspent_reservation_returns_to_the_budget() {
        let history = vec![turn("user", "curta"), turn("assistant", "resposta")];
        let mut budget = Budget { remaining: 8192 };

        fit_history_reserving_memory(history, &mut budget, true);

        assert_eq!(budget.remaining, 8192 - "curta".len() - "resposta".len());
    }

    #[test]
    fn history_that_fits_stays_in_chronological_order() {
        let history = vec![
            turn("user", "primeira"),
            turn("assistant", "segunda"),
            turn("user", "terceira"),
        ];
        let mut budget = Budget { remaining: 10_000 };

        let kept = fit_history(history, &mut budget);

        let contents: Vec<&str> = kept.iter().map(|m| m.message.content.as_str()).collect();
        assert_eq!(contents, vec!["primeira", "segunda", "terceira"]);
    }

    #[test]
    fn the_retrieved_passages_sit_immediately_above_the_question() {
        let question = question_with_context(
            "continue a frase",
            &[],
            &[source_block("codigo.pdf", "Art. 968. A inscrição do")],
            false,
        );

        assert!(
            question.ends_with("continue a frase"),
            "a pergunta tem que ser a última coisa que o modelo lê"
        );
        assert!(question.contains("Art. 968. A inscrição do"));
        assert!(question.starts_with(CONTEXT_PREAMBLE));
    }

    #[test]
    fn a_question_without_retrieval_carries_no_preamble() {
        // Told to cite sources with none given, a small model invents them —
        // observed as "[fonte: GPT-3 informações geral]".
        assert_eq!(
            question_with_context("o que é rust?", &[], &[], false),
            "o que é rust?"
        );
    }

    #[test]
    fn a_recalled_turn_is_never_presented_as_a_document() {
        // MEM-06. The two groups carry different preambles because the document
        // one orders the model to cite a filename, which a past exchange has not
        // got — and a model told to cite a missing source invents one.
        let question = question_with_context(
            "e o prazo?",
            &[memory_block("Usuário: qual era o prazo?\nAssistente: trinta dias")],
            &[],
            false,
        );

        assert!(question.starts_with(MEMORY_PREAMBLE));
        assert!(!question.contains(CONTEXT_PREAMBLE));
        assert!(!question.contains("[fonte:"));
        assert!(question.contains("[conversa anterior]"));
        assert!(question.ends_with("e o prazo?"));
    }

    #[test]
    fn the_document_sits_closer_to_the_question_when_it_is_the_better_hit() {
        // Proximity to the question is what the model reads (AD-033). While the
        // document is the nearer hit, it keeps the position it always had.
        let question = question_with_context(
            "resuma",
            &[memory_block("Usuário: oi\nAssistente: olá")],
            &[source_block("contrato.pdf", "cláusula 4")],
            false,
        );

        let memory_at = question.find("[conversa anterior]").unwrap();
        let document_at = question.find("[fonte: contrato.pdf]").unwrap();
        assert!(
            memory_at < document_at,
            "com o documento à frente no ranking, ele é quem fica colado na pergunta"
        );
    }

    #[test]
    fn the_recalled_turn_takes_the_place_next_to_the_question_when_it_wins() {
        // AD-050. Measured failure: an unrelated Civil Code chunk at 0.3150 sat
        // against the question while the turn that held the answer, at 0.2817,
        // sat behind it — and the model answered from the Civil Code. When the
        // recall is the nearer hit, it is the recall that touches the question.
        let question = question_with_context(
            "qual era o apelido?",
            &[memory_block("Usuário: chame de Falcão Azul\nAssistente: combinado")],
            &[source_block("codigo.pdf", "das obrigações de dar")],
            true,
        );

        let memory_at = question.find("[conversa anterior]").unwrap();
        let document_at = question.find("[fonte: codigo.pdf]").unwrap();
        assert!(
            document_at < memory_at,
            "o acerto mais próximo é o que fica colado na pergunta, seja de que camada for"
        );
        assert!(question.ends_with("qual era o apelido?"));
    }

    #[test]
    fn a_lone_layer_keeps_the_position_next_to_the_question_either_way() {
        // The flag reorders two sections; with only one, it must not be able to
        // push it away from the question or drop it.
        for wins in [false, true] {
            let only_memory = question_with_context(
                "e daí?",
                &[memory_block("Usuário: a\nAssistente: b")],
                &[],
                wins,
            );
            assert!(only_memory.starts_with(MEMORY_PREAMBLE));
            assert!(only_memory.ends_with("e daí?"));

            let only_documents =
                question_with_context("e daí?", &[], &[source_block("x.pdf", "y")], wins);
            assert!(only_documents.starts_with(CONTEXT_PREAMBLE));
            assert!(only_documents.ends_with("e daí?"));
        }
    }

    #[test]
    fn no_hit_never_outranks_a_hit() {
        // `None` must not read as distance zero: a layer that found nothing
        // cannot win the position next to the question.
        assert!(nearer(Some(0.28), Some(0.31)));
        assert!(!nearer(Some(0.31), Some(0.28)));
        assert!(nearer(Some(0.9), None), "algum acerto ganha de nenhum");
        assert!(!nearer(None, Some(0.9)), "nenhum acerto não ganha de nada");
        assert!(!nearer(None, None));
    }

    #[test]
    fn with_memory_off_the_prompt_is_exactly_what_it_was_before() {
        // The regression guard for the whole feature: nothing about a chat with
        // the toggle off may differ from the pre-M6 prompt.
        let with_documents = question_with_context(
            "pergunta",
            &[],
            &[source_block("a.pdf", "trecho")],
            false,
        );
        assert_eq!(
            with_documents,
            format!("{CONTEXT_PREAMBLE}\n\n[fonte: a.pdf]\ntrecho\n\n---\n\npergunta")
        );
    }

    #[test]
    fn every_context_block_names_its_source() {
        let block = source_block("contrato.pdf", "o prazo é de 30 dias");
        assert!(block.starts_with("[fonte: contrato.pdf]"));
        assert!(block.contains("o prazo é de 30 dias"));
    }

    fn hit(answer_id: &str, text: &str) -> MemoryHit {
        MemoryHit {
            answer_id: answer_id.to_string(),
            text: text.to_string(),
        }
    }

    #[test]
    fn a_turn_already_in_the_prompt_is_not_recalled_on_top_of_itself() {
        // MEM-05: the exchange is still in the verbatim history, so retrieving
        // it again would send the same words twice in one prompt.
        let verbatim: std::collections::HashSet<&str> = ["answer-1"].into_iter().collect();
        let mut budget = Budget { remaining: 10_000 };

        let blocks = recall_blocks(
            vec![hit("answer-1", "turno recente"), hit("answer-9", "turno antigo")],
            &verbatim,
            &mut budget,
        );

        assert_eq!(blocks.len(), 1);
        assert!(blocks[0].contains("turno antigo"));
    }

    #[test]
    fn a_turn_the_budget_dropped_from_the_history_can_be_recalled() {
        // The exclusion set is what survived `fit_history`, not what was read
        // from the database — a turn the budget pushed out of the prompt is
        // precisely the one the memory exists to bring back.
        let verbatim: std::collections::HashSet<&str> = ["answer-recente"].into_iter().collect();
        let mut budget = Budget { remaining: 10_000 };

        let blocks = recall_blocks(vec![hit("answer-antigo", "o prazo é de 30 dias")], &verbatim, &mut budget);

        assert_eq!(blocks.len(), 1);
    }

    #[test]
    fn the_memory_is_what_a_tight_budget_gives_up_first() {
        // MEM-10/MEM-11: by the time the memory is served, the documents and
        // the recent turns have already taken what they needed. Nothing left
        // means no memory, and no error.
        let verbatim = std::collections::HashSet::new();
        let mut exhausted = Budget { remaining: 0 };

        let blocks = recall_blocks(vec![hit("a", "qualquer coisa")], &verbatim, &mut exhausted);

        assert!(blocks.is_empty(), "sem orçamento, a memória simplesmente não entra");
    }

    #[test]
    fn context_budget_leaves_room_for_the_answer() {
        let chars = budget_chars(Some(1024));
        assert_eq!(chars, (1024 - 512) * CHARS_PER_TOKEN);
        assert!(budget_chars(Some(100)) == 0, "a tiny window reserves nothing");
    }
}

/// Mede o defeito que a UAT da T9 encontrou, contra o banco vetorial real.
///
/// The measured failure (2026-07-27, driving the app through its own UI): a
/// twelve-turn conversation, the fact planted in turn 1 and backfilled into the
/// memory, and the question asked once. With **"usar meus documentos" off** the
/// answer was right — *"Falcão Azul … 82 mil reais"*. With it **on**, and the
/// only document in the base being an unrelated Civil Code PDF, the same
/// question in the same conversation shape produced *"Projeto de Código Civil
/// Brasileiro 115 … R$ 250.000,00"*. The document was not merely useless: it
/// displaced the recall.
///
/// The suspected mechanism is `rank_candidates`. Its floor is **relative to the
/// best hit of the same query**, so a namespace whose best hit is already
/// irrelevant still contributes: the bad best defines the cutoff and its equally
/// bad siblings clear it. Nothing in the funnel can say "this whole namespace
/// had nothing to offer".
///
/// This module measures rather than argues it. Both paths come from the
/// environment and are never guessed, because one of them points at the user's
/// own data:
///
/// ```text
/// LOCALMIND_EMBED_CACHE=<copy of models--intfloat--multilingual-e5-small's parent>
/// LOCALMIND_ORT_DYLIB=<path to onnxruntime.dll>
/// LOCALMIND_VECTORS_COPY=<copy of the user's vectors/ folder>
/// cargo test --lib chat::retrieval_quality -- --ignored --nocapture
/// ```
#[cfg(test)]
mod retrieval_quality {
    use super::*;
    use crate::rag::embedding;
    use crate::rag::store::VectorStore;

    /// The question from the UAT, verbatim. Changing it changes what is being
    /// measured, so it is a constant rather than an inline literal.
    const QUESTION: &str =
        "Voltando ao começo: qual foi o apelido que eu dei ao meu projeto e quanto foi liberado de verba?";

    /// The turn that was in the conversation's memory, serialized the way
    /// `chat::memory` stores it.
    fn planted_turn() -> String {
        crate::chat::memory::serialize_turn(
            "Guarde isto para o resto da conversa: o codinome do meu projeto é Falcão Azul \
             e o orçamento aprovado foi de 82 mil reais.",
            "Codinome do projeto: Falcão Azul\nOrçamento: 82 mil reais",
        )
    }

    fn squared_l2(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum()
    }

    #[tokio::test]
    #[ignore = "needs LOCALMIND_EMBED_CACHE, LOCALMIND_ORT_DYLIB and LOCALMIND_VECTORS_COPY"]
    async fn an_unrelated_document_still_clears_the_relative_floor() {
        let Ok(cache) = std::env::var("LOCALMIND_EMBED_CACHE") else {
            panic!("set LOCALMIND_EMBED_CACHE to a *copy* of the model cache");
        };
        let Ok(dylib) = std::env::var("LOCALMIND_ORT_DYLIB") else {
            panic!("set LOCALMIND_ORT_DYLIB to the onnxruntime shared library");
        };
        let Ok(vectors) = std::env::var("LOCALMIND_VECTORS_COPY") else {
            panic!("set LOCALMIND_VECTORS_COPY to a *copy* of the vectors folder — never the original");
        };
        std::env::set_var("ORT_DYLIB_PATH", &dylib);
        embedding::set_cache_dir(std::path::PathBuf::from(cache));

        let query = embedding::embed_query(QUESTION).expect("query embedding failed");
        let store = VectorStore::open(std::path::Path::new(&vectors))
            .await
            .expect("could not open the vectors copy");

        let hits = store
            .search(GLOBAL_NAMESPACE, &query, PER_NAMESPACE_K)
            .await
            .expect("global search failed");

        println!("\npergunta: {QUESTION}");
        println!("\n-- base global ({} candidatos) --", hits.len());
        for (rank, hit) in hits.iter().enumerate() {
            let head: String = hit.text.chars().take(90).collect();
            println!("  #{rank} {:.4}  {}", hit.distance, head.replace('\n', " "));
        }

        // The memory turn is embedded here rather than read from the store: the
        // conversation this came from was deleted when the UAT cleaned up after
        // itself, and the number that matters is the distance, not the row.
        let turn = planted_turn();
        let turn_vec = &embedding::embed_passages(&[turn.clone()]).expect("embedding failed")[0];
        let turn_distance = squared_l2(&query, turn_vec);
        println!("\n-- memória --");
        println!("  {:.4}  {}", turn_distance, turn.replace('\n', " | "));

        let Some(best_document) = hits.first().map(|h| h.distance) else {
            panic!("a base global está vazia nesta cópia — o teste não tem o que medir");
        };
        let cutoff = (best_document * RELATIVE_DISTANCE_FLOOR).max(MIN_DISTANCE_CUTOFF);
        let survivors = hits.iter().filter(|h| h.distance <= cutoff).count();

        println!("\n-- o funil como ele é hoje --");
        println!("  melhor documento .......... {best_document:.4}");
        println!("  turno da memória .......... {turn_distance:.4}");
        println!("  corte relativo (×{RELATIVE_DISTANCE_FLOOR}) ...... {cutoff:.4}");
        println!("  documentos que passam ..... {survivors} de {}", hits.len());
        println!(
            "  o documento está {:.2}× mais LONGE da pergunta que o turno da memória",
            turn_distance.max(0.0001).recip() * best_document
        );

        // The point of the measurement: the document block is built from hits
        // that are further from the question than the memory turn, and the
        // relative floor lets them through anyway.
        assert!(
            survivors > 0,
            "nenhum documento passou o corte — a falha medida na UAT não reproduz \
             com esta base, e este teste precisa de dados novos"
        );
        assert!(
            best_document > turn_distance,
            "o documento ficou MAIS PERTO que o turno da memória ({best_document:.4} \
             contra {turn_distance:.4}): a causa do defeito não é o piso relativo, \
             e o diagnóstico precisa ser refeito"
        );
    }

    /// Procura o limiar **absoluto** que separaria um acerto real de um trecho
    /// que só é o menos ruim de uma base que não tem a resposta.
    ///
    /// The first test above establishes that the relative floor cannot make that
    /// distinction: with no good hit anywhere, the best of a bad lot sets the bar
    /// and everything clears it. An absolute ceiling could — but only if the two
    /// populations actually sit in different bands, and the comment on
    /// `RELATIVE_DISTANCE_FLOOR` asserts they do not.
    ///
    /// So this measures both populations against the same corpus instead of
    /// arguing about them: questions the Civil Code genuinely answers, and
    /// questions it has nothing to do with. A threshold is only worth writing
    /// down if the worst real hit is nearer than the best irrelevant one.
    #[tokio::test]
    #[ignore = "needs LOCALMIND_EMBED_CACHE, LOCALMIND_ORT_DYLIB and LOCALMIND_VECTORS_COPY"]
    async fn the_gap_between_a_real_hit_and_the_least_bad_one_is_measured_not_assumed() {
        let Ok(cache) = std::env::var("LOCALMIND_EMBED_CACHE") else {
            panic!("set LOCALMIND_EMBED_CACHE to a *copy* of the model cache");
        };
        let Ok(dylib) = std::env::var("LOCALMIND_ORT_DYLIB") else {
            panic!("set LOCALMIND_ORT_DYLIB to the onnxruntime shared library");
        };
        let Ok(vectors) = std::env::var("LOCALMIND_VECTORS_COPY") else {
            panic!("set LOCALMIND_VECTORS_COPY to a *copy* of the vectors folder");
        };
        std::env::set_var("ORT_DYLIB_PATH", &dylib);
        embedding::set_cache_dir(std::path::PathBuf::from(cache));
        let store = VectorStore::open(std::path::Path::new(&vectors))
            .await
            .expect("could not open the vectors copy");

        // Questions the corpus answers. The corpus is the Brazilian Civil Code,
        // so these are all things it actually contains.
        let relevant = [
            "Qual é o prazo de prescrição da pretensão de reparação civil?",
            "O que caracteriza a posse de boa-fé?",
            "Quando começa a personalidade civil da pessoa natural?",
            "O que é usucapião extraordinária?",
        ];
        // Questions it has nothing to do with, of the shape a chat actually
        // produces -- including the one from the UAT.
        let irrelevant = [
            QUESTION,
            "Como faço arroz de forno?",
            "Qual é a fórmula química da água?",
            "Quem ganhou a Copa do Mundo de 2002?",
        ];

        let mut best_relevant: Vec<f32> = Vec::new();
        let mut best_irrelevant: Vec<f32> = Vec::new();

        println!("
== perguntas que o corpus responde ==");
        for q in relevant {
            let v = embedding::embed_query(q).expect("query embedding failed");
            let hits = store.search(GLOBAL_NAMESPACE, &v, 1).await.expect("search failed");
            let d = hits.first().map(|h| h.distance).unwrap_or(f32::NAN);
            best_relevant.push(d);
            println!("  {d:.4}  {q}");
        }

        println!("
== perguntas que ele não responde ==");
        for q in irrelevant {
            let v = embedding::embed_query(q).expect("query embedding failed");
            let hits = store.search(GLOBAL_NAMESPACE, &v, 1).await.expect("search failed");
            let d = hits.first().map(|h| h.distance).unwrap_or(f32::NAN);
            best_irrelevant.push(d);
            println!("  {d:.4}  {q}");
        }

        let worst_relevant = best_relevant.iter().cloned().fold(f32::MIN, f32::max);
        let best_of_bad = best_irrelevant.iter().cloned().fold(f32::MAX, f32::min);
        println!("
== a faixa ==");
        println!("  pior acerto real .............. {worst_relevant:.4}");
        println!("  melhor acerto irrelevante ..... {best_of_bad:.4}");
        if worst_relevant < best_of_bad {
            println!(
                "  SEPARAM-SE: qualquer limiar entre {worst_relevant:.4} e {best_of_bad:.4}                  corta os irrelevantes sem perder nenhum acerto real"
            );
        } else {
            println!(
                "  NÃO SE SEPARAM: as duas populações se sobrepõem, e um limiar absoluto                  perderia acerto real junto com o lixo"
            );
        }
    }
}

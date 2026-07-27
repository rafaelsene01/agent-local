// SPEC: conversation-memory (MEM-01, MEM-07, MEM-08, MEM-14, MEM-17, MEM-18, MEM-19, MEM-20)

use crate::db::{require_conn, DbState};
use crate::rag::store::{EmbeddedChunk, RetrievedChunk, VectorStore};
use crate::rag::{chunking, embedding, pipeline, CHUNK_MAX_TOKENS, CHUNK_OVERLAP_TOKENS};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

/// A conversation's memory lives in its own namespace, separate from the one
/// holding that same chat's attachments.
///
/// Sharing `chat:<id>` with the attachments would tie the two together in two
/// ways that have nothing to do with each other: the memory toggle would also
/// switch off the attachments, and the retrieval cap would be spent by whichever
/// of the two happened to score better — a long conversation would crowd out the
/// file the user just attached.
pub fn memory_namespace(chat_id: &str) -> String {
    format!("memory:{chat_id}")
}

/// The labels are what make a retrieved block readable as a past exchange
/// instead of an anonymous passage. They are part of what gets embedded on
/// purpose: the pair is stored the way it will be read.
pub fn serialize_turn(question: &str, answer: &str) -> String {
    format!("Usuário: {question}\nAssistente: {answer}")
}

/// One complete exchange. `answer_id` is the assistant message's own id, which
/// becomes the `doc_id` in the vector store — see `record_turn`.
#[derive(Debug, Clone, PartialEq)]
pub struct Turn {
    pub answer_id: String,
    pub question: String,
    pub answer: String,
}

/// A message as it sits in SQLite, before it is paired into a turn.
#[derive(Debug, Clone)]
pub struct StoredMessage {
    pub id: String,
    pub role: String,
    pub content: String,
}

/// Reduces a chronological message list to the complete exchanges in it.
///
/// Everything unpaired is dropped, and that is the point: a question whose
/// generation was cancelled has no answer, and an answer recovered without its
/// question is a statement with no subject. Retrieving either one later would
/// put a fragment in the prompt with the authority of a whole turn (MEM-03,
/// MEM-20).
pub fn pair_turns(messages: &[StoredMessage]) -> Vec<Turn> {
    let mut turns = Vec::new();
    let mut pending: Option<&StoredMessage> = None;

    for message in messages {
        match message.role.as_str() {
            // A second question in a row replaces the first: the earlier one
            // never got answered, so it is one of the unpaired ones.
            "user" => pending = Some(message),
            "assistant" => {
                if let Some(question) = pending.take() {
                    turns.push(Turn {
                        answer_id: message.id.clone(),
                        question: question.content.clone(),
                        answer: message.content.clone(),
                    });
                }
            }
            _ => {}
        }
    }
    turns
}

/// Whether this chat wants its conversation remembered.
///
/// A chat that no longer exists reads as "no" rather than as an error: the
/// caller is deciding whether to write memory, and a deleted chat must not get
/// any (C-14 — `delete_chat` still does not cancel an in-flight generation).
pub fn uses_memory(app: &AppHandle, chat_id: &str) -> bool {
    let db = app.state::<DbState>();
    let Ok(guard) = db.0.lock() else { return false };
    let Some(sql) = guard.as_ref() else {
        return false;
    };
    sql.query_row(
        "SELECT use_memory FROM chats WHERE id = ?1",
        params![chat_id],
        |row| row.get::<_, i64>(0),
    )
    .optional()
    .map(|found| found == Some(1))
    .unwrap_or(false)
}

/// Embeds one exchange and writes it into the chat's memory.
///
/// `doc_id` is the assistant message's id rather than a fresh UUID, and that
/// single choice is what makes re-indexing safe: `upsert` deletes by `doc_id`
/// before writing, so recording a turn that the backfill later re-processes
/// leaves one copy, not two (MEM-19).
pub async fn record_turn(app: &AppHandle, chat_id: &str, turn: &Turn) -> Result<(), String> {
    let text = serialize_turn(&turn.question, &turn.answer);
    // A long exchange can exceed the embedding model's window, so it is sliced
    // with the same chunker documents use rather than truncated.
    let chunks = chunking::chunk_text(&text, CHUNK_MAX_TOKENS, CHUNK_OVERLAP_TOKENS);
    if chunks.is_empty() {
        return Ok(());
    }

    crate::rag::onnxruntime::ensure_dylib(app).await?;
    let texts: Vec<String> = chunks.iter().map(|c| c.text.clone()).collect();
    let vectors = tauri::async_runtime::spawn_blocking(move || embedding::embed_passages(&texts))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;

    let embedded: Vec<EmbeddedChunk> = chunks
        .iter()
        .zip(vectors)
        .map(|(chunk, vector)| EmbeddedChunk {
            id: format!("{}#{}", turn.answer_id, chunk.index),
            text: chunk.text.clone(),
            vector,
            chunk_index: chunk.index as i32,
        })
        .collect();

    let store = VectorStore::open(&pipeline::vectors_dir(app)?)
        .await
        .map_err(|e| e.to_string())?;

    // Last check before the write. A chat deleted mid-generation would
    // otherwise leave vectors in a namespace nothing will ever clean up,
    // because `delete_chat` already ran.
    if !chat_exists(app, chat_id) {
        return Ok(());
    }

    store
        .upsert(&memory_namespace(chat_id), &turn.answer_id, embedded)
        .await
        .map_err(|e| e.to_string())
}

/// Searches only this conversation's memory (MEM-08).
///
/// The namespace is built here rather than taken from the caller so that no
/// call site can accidentally pass another chat's namespace — the isolation
/// the user asked for is a property of this function, not of its callers.
pub async fn search(
    store: &VectorStore,
    chat_id: &str,
    query_vec: &[f32],
    top_k: usize,
) -> Result<Vec<RetrievedChunk>, String> {
    store
        .search(&memory_namespace(chat_id), query_vec, top_k)
        .await
        .map_err(|e| e.to_string())
}

pub fn chat_exists(app: &AppHandle, chat_id: &str) -> bool {
    let db = app.state::<DbState>();
    let Ok(guard) = db.0.lock() else { return false };
    let Some(sql) = guard.as_ref() else {
        return false;
    };
    sql.query_row("SELECT 1 FROM chats WHERE id = ?1", params![chat_id], |_| {
        Ok(())
    })
    .optional()
    .map(|found| found.is_some())
    .unwrap_or(false)
}

/// Progress of an on-demand backfill. Same shape as `DocumentStatusEvent`: the
/// UI already knows how to show a count that climbs to a total (MEM-18).
#[derive(Debug, Clone, Serialize)]
pub struct MemoryBackfillProgress {
    pub chat_id: String,
    pub done: usize,
    pub total: usize,
}

fn stored_messages(app: &AppHandle, chat_id: &str) -> Result<Vec<StoredMessage>, String> {
    let db = app.state::<DbState>();
    let guard = db.0.lock().map_err(|e| e.to_string())?;
    let sql = require_conn(&guard)?;
    let mut stmt = sql
        .prepare(
            "SELECT id, role, content FROM messages WHERE chat_id = ?1 ORDER BY created_at ASC",
        )
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map(params![chat_id], |row| {
            Ok(StoredMessage {
                id: row.get(0)?,
                role: row.get(1)?,
                content: row.get(2)?,
            })
        })
        .map_err(|e| e.to_string())?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| e.to_string())?;
    Ok(rows)
}

/// Indexes the exchanges already stored in a conversation (MEM-17).
///
/// Runs only when asked. A boot-time sweep was rejected during planning: on a
/// large history it is minutes of embedding CPU right after an update, which
/// reads as the app having frozen.
pub async fn backfill(app: &AppHandle, chat_id: &str) -> Result<usize, String> {
    if !uses_memory(app, chat_id) {
        return Err("A memória desta conversa está desligada — ligue-a antes de indexar o histórico."
            .to_string());
    }

    let turns = pair_turns(&stored_messages(app, chat_id)?);
    let total = turns.len();
    let _ = app.emit(
        "memory-backfill-progress",
        MemoryBackfillProgress {
            chat_id: chat_id.to_string(),
            done: 0,
            total,
        },
    );

    for (done, turn) in turns.iter().enumerate() {
        // A chat deleted mid-backfill stops the work instead of writing into a
        // namespace that `delete_chat` has already cleaned.
        if !chat_exists(app, chat_id) {
            return Ok(done);
        }
        record_turn(app, chat_id, turn).await?;
        let _ = app.emit(
            "memory-backfill-progress",
            MemoryBackfillProgress {
                chat_id: chat_id.to_string(),
                done: done + 1,
                total,
            },
        );
    }

    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rag::store::{chat_namespace, GLOBAL_NAMESPACE};

    fn message(id: &str, role: &str, content: &str) -> StoredMessage {
        StoredMessage {
            id: id.to_string(),
            role: role.to_string(),
            content: content.to_string(),
        }
    }

    /// MEM-07 as an assertion rather than as a property of the string format:
    /// the three namespaces have to stay disjoint for the isolation to hold.
    #[test]
    fn the_memory_namespace_never_collides_with_attachments_or_the_global_base() {
        assert_eq!(memory_namespace("abc"), "memory:abc");
        assert_ne!(memory_namespace("abc"), chat_namespace("abc"));
        assert_ne!(memory_namespace("abc"), GLOBAL_NAMESPACE);
        // A chat literally named "global" must not reach the global base.
        assert_ne!(memory_namespace("global"), GLOBAL_NAMESPACE);
    }

    #[test]
    fn a_turn_is_stored_with_both_sides_labelled() {
        let text = serialize_turn("qual é o prazo?", "trinta dias");
        assert!(text.contains("Usuário: qual é o prazo?"));
        assert!(text.contains("Assistente: trinta dias"));
    }

    #[test]
    fn only_complete_exchanges_become_turns() {
        let turns = pair_turns(&[
            message("m1", "user", "primeira pergunta"),
            message("m2", "assistant", "primeira resposta"),
            message("m3", "user", "segunda pergunta"),
            message("m4", "assistant", "segunda resposta"),
        ]);

        assert_eq!(turns.len(), 2);
        assert_eq!(turns[0].answer_id, "m2");
        assert_eq!(turns[0].question, "primeira pergunta");
        assert_eq!(turns[1].answer_id, "m4");
    }

    #[test]
    fn a_cancelled_generation_leaves_no_turn_behind() {
        // The user asked twice; the first generation was cancelled, so only the
        // second question has an answer.
        let turns = pair_turns(&[
            message("m1", "user", "pergunta abandonada"),
            message("m2", "user", "pergunta respondida"),
            message("m3", "assistant", "resposta"),
        ]);

        assert_eq!(turns.len(), 1);
        assert_eq!(turns[0].question, "pergunta respondida");
    }

    #[test]
    fn an_answer_without_a_question_is_not_a_turn() {
        let turns = pair_turns(&[
            message("m1", "assistant", "resposta órfã"),
            message("m2", "system", "regras"),
        ]);
        assert!(turns.is_empty());
    }

    #[test]
    fn a_conversation_with_nothing_finished_produces_no_turns() {
        // MEM-20: the backfill of such a chat has nothing to do, and that is a
        // normal outcome rather than an error.
        assert!(pair_turns(&[message("m1", "user", "só uma pergunta")]).is_empty());
        assert!(pair_turns(&[]).is_empty());
    }

    /// Recording the same exchange twice must leave one copy. The guarantee
    /// comes from `doc_id` being the assistant message id, which `upsert`
    /// deletes by before writing — a claim about LanceDB's behaviour, so it is
    /// checked against a real table rather than read off the code.
    #[tokio::test]
    #[ignore = "writes a real LanceDB table to a temp folder"]
    async fn re_indexing_a_turn_replaces_it_instead_of_duplicating_it() {
        use crate::rag::embedding::EMBEDDING_DIM;

        let dir = std::env::temp_dir().join(format!("localmind-memory-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = VectorStore::open(&dir).await.unwrap();

        let chunk = |text: &str| EmbeddedChunk {
            id: "answer-1#0".to_string(),
            text: text.to_string(),
            vector: vec![0.1; EMBEDDING_DIM],
            chunk_index: 0,
        };

        let namespace = memory_namespace("chat-1");
        store
            .upsert(&namespace, "answer-1", vec![chunk("primeira gravação")])
            .await
            .unwrap();
        store
            .upsert(&namespace, "answer-1", vec![chunk("segunda gravação")])
            .await
            .unwrap();

        let hits = store
            .search(&namespace, &vec![0.1; EMBEDDING_DIM], 10)
            .await
            .unwrap();
        assert_eq!(hits.len(), 1, "the same turn must not be stored twice");
        assert_eq!(hits[0].text, "segunda gravação");

        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The isolation the user asked for, against a real store (MEM-07, MEM-08,
    /// MEM-09).
    ///
    /// It exercises the two store calls `delete_chat` makes, not the command
    /// itself — the command needs an `AppHandle`, which no test here can build.
    /// What this proves is that the namespaces are disjoint and that deleting
    /// one leaves the other whole; that `delete_chat` calls both is a claim the
    /// code makes and only the UAT can close.
    #[tokio::test]
    #[ignore = "writes a real LanceDB table to a temp folder"]
    async fn one_conversation_never_recalls_another_and_deleting_it_spares_the_rest() {
        use crate::rag::embedding::EMBEDDING_DIM;

        let dir = std::env::temp_dir().join(format!("localmind-isolation-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let store = VectorStore::open(&dir).await.unwrap();

        let turn = |id: &str, text: &str, seed: f32| EmbeddedChunk {
            id: id.to_string(),
            text: text.to_string(),
            vector: vec![seed; EMBEDDING_DIM],
            chunk_index: 0,
        };

        store
            .upsert(
                &memory_namespace("chat-a"),
                "answer-a",
                vec![turn("a#0", "Usuário: qual o codinome?\nAssistente: pantera", 0.1)],
            )
            .await
            .unwrap();
        store
            .upsert(
                &memory_namespace("chat-b"),
                "answer-b",
                vec![turn("b#0", "Usuário: bom dia\nAssistente: bom dia", 0.1)],
            )
            .await
            .unwrap();
        // The same chat's attachments must not be reachable through its memory
        // namespace either — that is the reason the two are separate.
        store
            .upsert(
                &chat_namespace("chat-a"),
                "anexo-a",
                vec![turn("att#0", "conteúdo de um anexo", 0.1)],
            )
            .await
            .unwrap();

        let query = vec![0.1f32; EMBEDDING_DIM];

        let from_b = search(&store, "chat-b", &query, 10).await.unwrap();
        assert_eq!(from_b.len(), 1, "chat B só pode ver a própria memória");
        assert!(
            !from_b[0].text.contains("pantera"),
            "o termo exclusivo de A não pode chegar a B"
        );

        let from_a = search(&store, "chat-a", &query, 10).await.unwrap();
        assert_eq!(from_a.len(), 1, "a memória não pode enxergar os anexos");
        assert!(from_a[0].text.contains("pantera"));

        // What `delete_chat` does: both namespaces of the deleted chat, nothing
        // belonging to anyone else.
        store
            .delete_namespace(&chat_namespace("chat-a"))
            .await
            .unwrap();
        store
            .delete_namespace(&memory_namespace("chat-a"))
            .await
            .unwrap();

        assert!(search(&store, "chat-a", &query, 10).await.unwrap().is_empty());
        assert_eq!(
            search(&store, "chat-b", &query, 10).await.unwrap().len(),
            1,
            "apagar uma conversa não pode encostar na outra"
        );

        let _ = std::fs::remove_dir_all(&dir);
    }
}

/// Answers the design's Open Question #1 — *is a role-labelled turn good
/// material for this embedding model?* — against the real model instead of on
/// paper.
///
/// The model is multilingual-e5-small, trained on passages, not on dialogue. A
/// stored turn carries `Usuário:` / `Assistente:` markers that no document has,
/// and the honest answer before running this was "unknown".
///
/// Both paths come from the environment and are never guessed, because one of
/// them would otherwise point at the user's own model folder:
///
/// ```text
/// LOCALMIND_EMBED_CACHE=<copy of models--intfloat--multilingual-e5-small's parent>
/// LOCALMIND_ORT_DYLIB=<path to onnxruntime.dll>
/// cargo test --lib chat::memory_quality -- --ignored --nocapture
/// ```
#[cfg(test)]
mod memory_quality {
    use super::*;
    use crate::rag::embedding;

    /// LanceDB ranks by squared L2 on these vectors, so the comparison here has
    /// to use the same metric the retrieval will — cosine would rank correctly
    /// and still say nothing about the cutoff in `rank_candidates`.
    fn squared_l2(a: &[f32], b: &[f32]) -> f32 {
        a.iter().zip(b).map(|(x, y)| (x - y) * (x - y)).sum()
    }

    #[test]
    #[ignore = "needs LOCALMIND_EMBED_CACHE and LOCALMIND_ORT_DYLIB on a machine with the model"]
    fn a_labelled_turn_is_retrievable_by_what_was_said_in_it() {
        let Ok(cache) = std::env::var("LOCALMIND_EMBED_CACHE") else {
            panic!("set LOCALMIND_EMBED_CACHE to a *copy* of the model cache");
        };
        let Ok(dylib) = std::env::var("LOCALMIND_ORT_DYLIB") else {
            panic!("set LOCALMIND_ORT_DYLIB to the onnxruntime shared library");
        };
        std::env::set_var("ORT_DYLIB_PATH", &dylib);
        embedding::set_cache_dir(std::path::PathBuf::from(cache));

        // A conversation the way it is actually stored: complete turns, both
        // sides labelled.
        let turns = [
            serialize_turn(
                "qual é o prazo de entrega que combinamos?",
                "combinamos trinta dias corridos a partir da assinatura",
            ),
            serialize_turn(
                "me explica o que é um índice em banco de dados",
                "é uma estrutura que acelera a busca por uma coluna, ao custo de escrita mais lenta",
            ),
            serialize_turn(
                "qual a capital da Austrália?",
                "Camberra, e não Sydney, que é a cidade mais populosa",
            ),
        ];

        // The same three exchanges with the role markers stripped — the design's
        // plan B. Measured side by side because the question is not "does it
        // work" but "do the labels cost separation".
        let unlabelled: Vec<String> = turns
            .iter()
            .map(|t| {
                t.lines()
                    .map(|l| {
                        l.trim_start_matches("Usuário: ")
                            .trim_start_matches("Assistente: ")
                    })
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect();

        let passages: Vec<String> = turns.to_vec();
        let vectors = embedding::embed_passages(&passages).expect("embedding failed");
        let bare = embedding::embed_passages(&unlabelled).expect("embedding failed");

        // Asked much later, with none of the words of the stored answer except
        // "prazo" — this is the shape of a real follow-up question.
        let question = "o prazo que a gente tinha acertado era de quanto tempo?";
        let query = embedding::embed_query(question).expect("query embedding failed");

        let mut scored: Vec<(usize, f32)> = vectors
            .iter()
            .enumerate()
            .map(|(i, v)| (i, squared_l2(&query, v)))
            .collect();
        scored.sort_by(|a, b| a.1.total_cmp(&b.1));

        println!("\npergunta: {question}");
        for (i, distance) in &scored {
            let head: String = turns[*i].lines().next().unwrap_or("").to_string();
            println!("  {distance:.4}  {head}");
        }

        let best = scored[0];
        let runner_up = scored[1];
        assert_eq!(
            best.0, 0,
            "o turno sobre o prazo tem que ganhar do turno sobre índices e do de geografia"
        );

        // The floor that decides what reaches the prompt is
        // `RELATIVE_DISTANCE_FLOOR` (3.0) applied to the best hit. If the
        // runner-up sits under that multiple, every question drags two extra
        // turns into the prompt regardless of relevance — which is the
        // failure this number exists to expose.
        println!(
            "\ncom rótulos:  melhor {:.4} · segundo {:.4} · razão {:.2}×",
            best.1,
            runner_up.1,
            runner_up.1 / best.1.max(0.0001)
        );

        let mut bare_scored: Vec<(usize, f32)> = bare
            .iter()
            .enumerate()
            .map(|(i, v)| (i, squared_l2(&query, v)))
            .collect();
        bare_scored.sort_by(|a, b| a.1.total_cmp(&b.1));
        println!(
            "sem rótulos:  melhor {:.4} · segundo {:.4} · razão {:.2}×  (vencedor: turno {})",
            bare_scored[0].1,
            bare_scored[1].1,
            bare_scored[1].1 / bare_scored[0].1.max(0.0001),
            bare_scored[0].0
        );
        println!("o piso relativo corta em 3,00× — abaixo disso, nada é filtrado\n");

        // The case that decides whether the floor is worth anything here: a
        // question about something the conversation never touched. If the
        // nearest turn still passes the cutoff, then every message drags
        // irrelevant turns into the prompt — and AD-033 measured what happens
        // when the model finds unrelated text next to the question.
        let stranger = "como faço arroz de forno?";
        let stranger_vec = embedding::embed_query(stranger).expect("query embedding failed");
        let mut off_topic: Vec<f32> = vectors
            .iter()
            .map(|v| squared_l2(&stranger_vec, v))
            .collect();
        off_topic.sort_by(|a, b| a.total_cmp(b));
        let cutoff = (off_topic[0] * 3.0).max(0.1);
        let survivors = off_topic.iter().filter(|d| **d <= cutoff).count();
        println!("pergunta sem relação: {stranger}");
        println!(
            "  mais próximo {:.4} · corte {:.4} · {} de {} turnos passam o piso",
            off_topic[0],
            cutoff,
            survivors,
            off_topic.len()
        );
    }

    /// Reproduces the failure AD-047 found in the T9 conversation, against the
    /// real embedding model, and measures whether the fix actually clears it.
    ///
    /// The shape of that failure: the user plants a fact early, asks about it
    /// much later, gets nothing, and rephrases. **The rephrased question's
    /// nearest neighbour is the user's own previous question**, because a
    /// question is the nearest neighbour of itself. That previous turn is
    /// already quoted verbatim in the recent history, so the dedup filter drops
    /// it — and with the old funnel (`search` asked for exactly `MEMORY_TOP_K`
    /// candidates, filter applied *after* the cut) nothing was left.
    ///
    /// The instrumented run recorded it exactly:
    ///
    /// ```text
    /// DIAG recall: 1 hit(s), budget 54044
    /// DIAG   hit f8636416… verbatim=true text="Usuário: Voltando ao inicio: com que apelido eu batizei…"
    /// ```
    ///
    /// This test asserts the two halves separately, because they can regress
    /// independently: that the decoy really does outrank the planted turn (the
    /// cause is real, not a story), and that the planted turn is nonetheless
    /// within `MEMORY_CANDIDATES` (the fix is sufficient).
    #[test]
    #[ignore = "needs LOCALMIND_EMBED_CACHE and LOCALMIND_ORT_DYLIB on a machine with the model"]
    fn a_rephrased_question_still_reaches_the_turn_it_is_asking_about() {
        let Ok(cache) = std::env::var("LOCALMIND_EMBED_CACHE") else {
            panic!("set LOCALMIND_EMBED_CACHE to a *copy* of the model cache");
        };
        let Ok(dylib) = std::env::var("LOCALMIND_ORT_DYLIB") else {
            panic!("set LOCALMIND_ORT_DYLIB to the onnxruntime shared library");
        };
        std::env::set_var("ORT_DYLIB_PATH", &dylib);
        embedding::set_cache_dir(std::path::PathBuf::from(cache));

        // Turn 0 is the planted fact. The last turn is the decoy: the user's
        // earlier attempt to ask about it, stored with the model's refusal —
        // which is what a failed recall leaves behind in a real conversation.
        let planted = serialize_turn(
            "vou te dar um codinome pra esse projeto: chama ele de Albatroz daqui pra frente",
            "combinado, vou chamar o projeto de Albatroz",
        );
        let filler = [
            serialize_turn(
                "me explica o que é um índice em banco de dados",
                "é uma estrutura que acelera a busca por uma coluna, ao custo de escrita mais lenta",
            ),
            serialize_turn(
                "e qual a diferença entre índice único e composto?",
                "o único proíbe repetição na coluna; o composto cobre mais de uma coluna na mesma ordem",
            ),
            serialize_turn(
                "qual a capital da Austrália?",
                "Camberra, e não Sydney, que é a cidade mais populosa",
            ),
        ];
        let decoy = serialize_turn(
            "voltando ao início: com que apelido eu batizei o projeto?",
            "não tenho a capacidade de acessar informações pessoais",
        );

        let mut passages = vec![planted.clone()];
        passages.extend(filler.iter().cloned());
        passages.push(decoy.clone());
        let vectors = embedding::embed_passages(&passages).expect("embedding failed");

        // The rephrasing. Deliberately shares no word with the planted answer
        // except the topic itself.
        let question = "qual era mesmo o nome que eu dei pro projeto?";
        let query = embedding::embed_query(question).expect("query embedding failed");

        let mut scored: Vec<(usize, f32)> = vectors
            .iter()
            .enumerate()
            .map(|(i, v)| (i, squared_l2(&query, v)))
            .collect();
        scored.sort_by(|a, b| a.1.total_cmp(&b.1));

        let planted_idx = 0usize;
        let decoy_idx = passages.len() - 1;
        let rank_of = |idx: usize| scored.iter().position(|(i, _)| *i == idx).unwrap();

        println!("\npergunta reformulada: {question}");
        for (rank, (i, distance)) in scored.iter().enumerate() {
            let tag = if *i == planted_idx {
                " <- turno plantado"
            } else if *i == decoy_idx {
                " <- isca (já citada verbatim)"
            } else {
                ""
            };
            let head: String = passages[*i].lines().next().unwrap_or("").to_string();
            println!("  #{rank} {distance:.4}  {head}{tag}");
        }
        println!(
            "\nposição do plantado: #{} · da isca: #{} · MEMORY_CANDIDATES = 8, MEMORY_TOP_K = 1\n",
            rank_of(planted_idx),
            rank_of(decoy_idx)
        );

        // If this ever stops holding, the bug AD-047 fixed can no longer
        // reproduce with these strings — and this test would be passing for the
        // wrong reason, exactly like the pruning test in AD-046. Fail loudly
        // instead, so whoever sees it picks a decoy that ranks first again.
        assert!(
            rank_of(decoy_idx) < rank_of(planted_idx),
            "a isca deixou de ganhar do turno plantado: este teste não está mais \
             exercitando a falha da AD-047 e precisa de dados novos"
        );

        // The actual guarantee. `recall_blocks` drops the decoy as verbatim, so
        // the planted turn only reaches the prompt if the pool went deep enough
        // to contain it.
        assert!(
            rank_of(planted_idx) < 8,
            "o turno plantado ficou em #{}, fora dos 8 candidatos — a memória \
             devolveria vazio como na T9",
            rank_of(planted_idx)
        );
    }
}

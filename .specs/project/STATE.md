# State

**Last Updated:** 2026-07-25
**Current Work:** Todas as features planejadas estão implementadas (2026-07-25): M3.1 (10/10), M7 (16/16), M5 `documents-rag` (11/11) e M4 `chat-messaging` (12/12). 58 testes Rust verdes + 4 `#[ignore]` que exercitam ONNX/LanceDB de verdade. Falta: verificação clicando na UI (listada em Todos) e os milestones M6 (memória de conversa) e M8 (empacotamento), que ainda não têm spec. Contexto anterior: M3.1 e M7 implementados em 2026-07-25 — 38 testes Rust verdes, `npm run build` e `npm run tauri dev` limpos, sidecar llama.cpp exercitado de verdade (ver AD-024). Pendente nos dois: os passos que exigem clicar na UI. Próximo: M5 (`documents-rag`, 11 tasks), depois M4 (`chat-messaging`, 12 tasks). Mapeamento brownfield completo (`.specs/codebase/`, 7 docs). Dois planejamentos prontos para Execute, **nesta ordem obrigatória**: (1) `single-active-connection` — spec + tasks (10 tasks), regra nova de uma conexão/um modelo ativos; (2) `embedded-runtime` (M7) — spec + context + design + tasks (16 tasks), llama.cpp embutido. `documents-rag` (M5) e `chat-messaging` (M4) vêm depois.

---

## Recent Decisions (Last 60 days)

### AD-026: M4 (chat-messaging) implementado — 12/12 tasks (2026-07-25)

**Decision:** Executado o `tasks.md` completo. `ProviderClient` ganhou `stream_chat`; Ollama usa NDJSON próprio e LM Studio/custom/embedded compartilham **um** parser SSE (`providers/openai_stream.rs`) em vez de três cópias. `chat_commands::send_message` persiste a mensagem, ingere anexos, monta contexto e emite `chat-stream-chunk`; `CancellationRegistry` para o loop entre tokens.
**Reason:** Último item da fila de Todos; o usuário pediu "executa specs que falta e depois valide".
**Trade-off/Notas:**
- Anexo pequeno (≤8000 chars) entra inteiro no prompt; acima disso reusa `rag::pipeline::process_document` com `namespace = "chat:<id>"` (AD-017), **aguardado** antes de responder, porque a pergunta atual é justamente a que precisa dele.
- O pipeline registra estado na tabela `documents`; o anexo grande cria uma linha temporária ali, que é removida ao fim — `chat_attachments` é o registro definitivo. Não estava no design, é a consequência de reusar o pipeline.
- Cancelamento e erro de provedor **preservam o parcial**: o usuário fica com o que já viu na tela.
- Orçamento de contexto trunca a categoria que estoura em vez de descartá-la (CHAT-15), e a pergunta atual nunca é truncada.
- **Não verificado**: enviar mensagem de verdade pela UI, perguntar sobre um anexo, confirmar isolamento entre chats e a limpeza do `tmp/` ao excluir o chat (T12).

### AD-025: M5 (documents-rag) implementado — 11/11 tasks (2026-07-25)

**Decision:** Executado o `tasks.md` completo. `rag/` novo (`parsing`, `chunking`, `embedding`, `store`, `pipeline`, `onnxruntime`), `document_commands.rs`, `DocumentsPanel` e reenfileiramento no boot.
**Reason:** Pré-requisito real do M4 (AD-017).
**Pesquisa obrigatória cumprida (crates confirmados na crates.io no dia):** `pdf-extract` 0.12, `docx-rs` 0.4.22 (o `dotext` do design foi **rejeitado** — último release de 2017), `fastembed` 5.17 com `MultilingualE5Small` (a UI é EN+PT, modelo só-inglês recupera mal português), `lancedb` 0.31.
**Dois bloqueios de ambiente resolvidos:**
- `lancedb` exige o compilador **protoc** no build. Instalado via `winget install Google.Protobuf` (35.1) com aprovação do usuário — vira pré-requisito de build documentado.
- O ONNX Runtime estático do `fastembed` exige a STL do MSVC 2022; a máquina só tem VS 2019 Build Tools. Escolha do usuário: `ort-load-dynamic` + download do `onnxruntime.dll` em runtime (`rag/onnxruntime.rs`), mesmo padrão do sidecar llama.cpp.
**Bug real encontrado na validação:** dois documentos indexando ao mesmo tempo (DOC-07 permite explicitamente) inicializavam o modelo em paralelo e corrompiam o cache (`Failed to retrieve onnx/model.onnx`). Corrigido serializando a init com double-check.
**Verificação real (não só compilação):** embeddings via ONNX Runtime de verdade — paráfrase 0,957 vs texto não relacionado 0,826; pergunta em PT casa com passagem em EN 0,774 vs 0,683 (justifica o modelo multilíngue). LanceDB em disco: namespace do chat não vê o global, `delete_namespace` e `delete_by_doc` removem só o alvo, busca em base vazia devolve lista vazia. Banco real do usuário migrado até `user_version = 5` sem perder dados.
**Não verificado:** importar um documento clicando na UI e ver o progresso até "ready".

### AD-024: M7 (embedded-runtime) implementado — 16/16 tasks (2026-07-25)

**Decision:** Executado o `tasks.md` completo de `embedded-runtime` (T1-T16). Módulo `runtime/` novo (`release`, `download`, `detect`, `model`, `process`, `store`), `providers::embedded::EmbeddedClient`, `embedded_commands.rs`, conexão `embedded` semeada sempre, e `EmbeddedRuntimeCard` na aba Conexões.
**Reason:** Segundo item da fila, confirmado pelo usuário no escopo "M3.1 + M7".
**Verificação real (não só compilação):**
- Release `b10107` resolvido ao vivo pela API do GitHub; os 4 sufixos que o `pick_asset` casa existem de fato no release.
- URL do GGUF do Phi-3.5 (única incerteza declarada do design) confirmada: 200 + `content-length` 2.393.232.672 (~2,39 GB).
- Binário Vulkan baixado e extraído; `llama-server --list-devices` respondeu `Vulkan0: NVIDIA GeForce RTX 3060 (12329 MiB, 11550 MiB free)` — formato idêntico ao que o `classify_output` parseia.
- Sidecar subido com as flags exatas que o app monta (`-m`, `--host 127.0.0.1`, `--port`, `-ngl -1`): `/health` devolveu `{"status":"ok"}`, `/v1/models` listou o modelo (o que o `CustomClient` parseia) e `/v1/chat/completions` gerou resposta. 152 tok/s de geração e 498 tok/s de prompt confirmam que o offload de GPU funcionou (a 1ª chamada é lenta por compilação de pipeline Vulkan — não confundir com CPU).
**Trade-off/Notas:**
- `runtime/store.rs` não estava no design: a linha singleton `embedded_runtime` é lida tanto pelo comando quanto pelo autostart do boot, então o SQL ficou num módulo só (SPEC_DEVIATION no commit).
- `ConnectionManager` deixou de ser unit struct e passou a carregar um `EmbeddedContext` (porta + models_dir), porque a URL da conexão embutida só existe depois que o processo escolhe a porta. Todos os comandos que criam provider passaram a receber `AppHandle` e a construir o manager via `embedded_commands::manager`.
- T2 pedia baixar o timeout do client de 5s para 2s; feito **por requisição de health check**, porque o mesmo client serve downloads de vários GB.
- **Não verificado**: setup disparado pelo card na UI, fechar o app e confirmar que o `llama-server` sumiu, e reabrir com a conexão ativa para ver o autostart. O mecanismo (`RunEvent::ExitRequested` → `kill`) está no código e o `kill` também roda no `Drop`, mas nenhum dos três foi exercitado clicando.
**Correção pós-auditoria (mesmo dia):** uma revisão requisito-a-requisito encontrou dois itens marcados como prontos que não estavam:
- **EMBED-12 estava quebrado**: `configure_model` gravava contexto/GPU em `model_configs`, mas o sidecar inicia a partir da linha `embedded_runtime` — a configuração era persistida e ignorada. Corrigido: o provider embutido grava também na própria linha e reinicia o servidor se estiver rodando. Offload de GPU é tudo-ou-nada (`-ngl` quer contagem de camadas, que não dá pra saber sem ler o GGUF; fração vira "off", nunca "max" silencioso).
- **EMBED-04 AC4 incompleto**: o setup terminava em "pronto para iniciar" e exigia um segundo clique. Agora sobe o sidecar ao fim da instalação.
- **Desvio consciente mantido (EMBED-02)**: o AC diz que *ativar* a conexão dispara o download; a UI exige clique explícito em "Baixar e instalar", porque ativar por rádio não deveria começar um download de 2,4 GB. Se o comportamento literal for desejado, é uma mudança pequena no card.
**Impact:** M7 ✅ no ROADMAP; C-01, C-02 e C-07 do CONCERNS.md resolvidos; C-05 parcialmente (este é o primeiro provider exercitado contra um servidor real).

### AD-023: M3.1 (single-active-connection) implementado — 10/10 tasks (2026-07-25)

**Decision:** Executado o `tasks.md` completo de `single-active-connection` (T1-T10). `db.rs` passou a aplicar migrações versionadas por `PRAGMA user_version` (migração 1 = schema antigo, migração 2 = `enabled` → `is_active` + normalização); `toggle_connection` saiu do backend, do `lib.rs`, da API e da store; `get_active_model` virou `get_active_pair`; a UI trocou checkbox por radio exclusivo e passou a listar modelos de toda conexão disponível.
**Reason:** Primeiro item da fila de Todos, confirmado pelo usuário como escopo "M3.1 + M7".
**Trade-off/Notas:**
- **T3 e T4 num só commit**: o gate de T3 é `cargo test connections::`, que não passa enquanto `connection_commands.rs` ainda chama a função removida — os callers tiveram que ir junto (SPEC_DEVIATION registrada no commit).
- `create_connection` perdeu o parâmetro `enabled`: mantê-lo permitiria criar uma segunda conexão ativa e furar justamente a invariante da feature. Ativação agora só existe via `set_active_connection`/`set_active_model`.
- `set_active_connection` foi partida em `apply_active_connection` (sem transação própria) + wrapper: o SQLite rejeita `BEGIN` aninhado e `set_active_model` já abre a sua para ativar o par atomicamente.
- `list_installed_models` já aceitava qualquer `connection_id` e não exigia conexão ativa — ACTIVE-08 não precisou de mudança no backend, só na store/UI.
- **Não verificado**: a UI não foi exercitada clicando (ativar Ollama → ativar LM Studio → trocar modelo). O app sobe (`Finished` + `Running`) e o build é limpo, mas o fluxo visual continua na lista de Todos.
**Impact:** M3.1 ✅ no ROADMAP; `single-active-connection/{spec,tasks}.md` marcados; AD-016 revogada também no `chat-messaging/{design,tasks}.md` (T10). C-01 do CONCERNS.md resolvido.

### AD-022: Runtime embutido usa Vulkan (não CUDA), e o próprio binário detecta a GPU (2026-07-25)

**Decision:** O M7 baixa o build **Vulkan** do llama.cpp (mais o build CPU só como fallback se o Vulkan nem executar). CUDA fica fora. A detecção de GPU é feita rodando `llama-server --list-devices` e lendo a saída — sem `wgpu`, `ash` ou `nvml`.
**Reason:** Um binário Vulkan cobre NVIDIA/AMD/Intel sem exigir toolkit instalado. CUDA obrigaria escolher entre `cuda-12.4` e `cuda-13.3`, casar com versão de driver e dobrar a matriz de download. Sobre a detecção: o binário já sabe responder a pergunta, e uma lib de GPU responderia "existe Vulkan", não "o llama.cpp consegue usar".
**Trade-off:** Usuário com NVIDIA de ponta perde ~35% em prompt processing (benchmarks 2026: RTX 5090 ~14.073 vs ~10.382 pp512); geração de token fica praticamente empatada (290 vs 264 tg128). Registrado como Deferred Idea.
**Impact:** `embedded-runtime/design.md` (Tech Decisions) e tasks T4/T6.

### AD-021: Uma conexão ativa e um modelo ativo, globais — revoga a AD-016 (2026-07-25)

**Decision:** Existe no máximo **uma** conexão ativa e **um** modelo ativo no app inteiro, e o modelo ativo sempre pertence à conexão ativa. Escolher um modelo ativa a conexão dona dele na mesma ação. Conexões inativas continuam listadas com status e com modelos inspecionáveis.
**Reason:** Pedido literal do usuário — *"conexão e modelo deve ter somente um único ativo, que é ele que deve ser usado na hora do chat"*. O M3 tinha deixado uma assimetria: modelo já era único, mas `connections.enabled` permitia várias habilitadas, sem resposta para "qual delas responde?".
**Trade-off:** **Revoga a AD-016** (modelo por chat com fallback global) — perguntado explicitamente, o usuário escolheu matar o override por chat. Perde-se flexibilidade de usar modelos diferentes em chats diferentes; ganha-se um modelo mental sem ambiguidade.
**Impact:** `connections.enabled` vira `is_active`; `toggle_connection` sai e entram `set_active_connection`/`clear_active_connection`; `get_active_model` vira `get_active_pair`. `chat-messaging/design.md` precisa perder `chats.model_config_id` (task T10 de `single-active-connection`).

### AD-020: Migração de schema versionada com `PRAGMA user_version` (2026-07-25)

**Decision:** `db.rs` passa de um `execute_batch(SCHEMA)` único para uma lista ordenada de migrações aplicadas conforme o `PRAGMA user_version`, cada uma em transação.
**Reason:** O schema atual é só `CREATE TABLE IF NOT EXISTS` — funciona para adicionar tabela, mas vira **no-op silencioso** para mudança de coluna em banco já existente (C-01 no CONCERNS.md). A AD-021 precisa justamente renomear `connections.enabled`, e o M7 adiciona `embedded_runtime` — as duas próximas features batem nesse limite.
**Trade-off:** ~40 linhas de infra a mais; nenhuma dependência nova (é recurso nativo do SQLite).
**Impact:** `single-active-connection` T1/T2; `embedded-runtime` T3 entra como migração 3.

---

## Recent Decisions (Last 60 days)

### AD-019: M3 (connections-models) implementado — 15/15 tasks (2026-07-25)

**Decision:** Executado o `tasks.md` completo de `connections-models` (T1-T15), do zero até `ConnectionsPanel` funcional no `App.tsx`. Repositório git inicializado nesta sessão (não existia antes) especificamente para viabilizar 1 commit atômico por task.
**Reason:** Próximo passo registrado em Todos desta mesma STATE.md; usuário confirmou escopo "feature inteira T1-T15 autônomo" e "inicializar git agora" via pergunta direta no início da sessão.
**Trade-off/Notas:**
- Nenhum Ollama/LM Studio rodando neste ambiente — `OllamaClient`/`LmStudioClient`/`CustomClient` foram verificados por `cargo check`/`cargo test` e pelos payloads exatos documentados (pesquisa web durante T5/T6), não por chamada real a um servidor. Endpoint fields para LM Studio (`context_length` snake_case, `offload_kv_cache_to_gpu` boolean) divergiam do que o `design.md` original supunha (`contextLength`/`gpuOffload` camelCase graduado) — corrigido, documentado como SPEC_DEVIATION no código.
- `models.rs` virou `models/mod.rs` (Chat/Message preservados) para caber `models::catalog`/`models::memory_estimate` no path exato do design.
- `tasks.md` tinha algumas lacunas de integração não explícitas em nenhuma task individual, preenchidas durante a execução (todas com nota SPEC_DEVIATION no commit correspondente): provider "custom" sem client (`providers::custom::CustomClient`, T7), `set_active_model`/`configure_model` descritos como recebendo um `model_config_id` que nem sempre existe ainda (resolvido com find-or-create por `connection_id`+`model_name`, T9), nenhum getter para "qual é o modelo ativo" (`get_active_model`, addendum pós-T9), e nenhuma task listada para colocar `ModelsList`/`ModelConfigForm` dentro do `ConnectionsPanel` (feito nos próprios commits de T13/T14, já que design.md só permite um lugar pra isso).
**Impact:** M3 completo no ROADMAP; `connections-models/tasks.md` e `spec.md` atualizados (checkboxes + tabela de rastreabilidade). 9 commits no backend Rust, 6 no frontend React, todos atômicos.

---

## Recent Decisions (Last 60 days)

### AD-018: Streaming de chat via evento Tauri, não retorno de comando (2026-07-25)

**Decision:** `send_message` retorna o `message_id` do usuário imediatamente; os tokens da resposta chegam via evento (`chat-stream-chunk`), não como retorno do comando.
**Reason:** Comandos Tauri são request/response; token-a-token exige push. Mesmo padrão já usado para progresso de download (M3) e indexação de documentos (M5) — consistência entre as três features planejadas nesta sessão.
**Trade-off:** Frontend precisa gerenciar estado de "mensagem sendo montada" via listener, não só via return de promise.
**Impact:** `chat-messaging/design.md` define `ChatStreamChunk`; `CancellationRegistry` (por `chat_id`) permite parar no meio.

### AD-017: RAG com namespace único reusado por Documentos e Chat (2026-07-25)

**Decision:** Uma única abstração `VectorStore` (LanceDB, coluna `namespace`) atende tanto a base global (`namespace="global"`, M5) quanto os anexos por chat (`namespace="chat:<id>"`, M4) — mesmo código, sem duplicar orquestração de parse→chunk→embed→store.
**Reason:** `chat-messaging` (M4) precisava do mesmíssimo pipeline de `documents-rag` (M5), só trocando o namespace; construir dois pipelines seria retrabalho e um risco de os dois divergirem.
**Trade-off:** `chat-messaging` tem dependência de implementação direta em `documents-rag` (não só de arquitetura) — a ordem de execução importa de verdade, não é só preferência de roadmap.
**Impact:** `tasks.md` de `chat-messaging` referencia tasks de `documents-rag` explicitamente como "Externo" nas dependências (T5, T6 dependem de documents-rag T3/T4/T5/T6).

### AD-016: ~~Modelo ativo é por chat, com fallback pro modelo global~~ — **REVOGADA em 2026-07-25 pela AD-021**

> Revogada no mesmo dia em que foi escrita, antes de qualquer código depender dela. O usuário decidiu que existe um único par ativo global (conexão + modelo) e que não há override por chat. `chats.model_config_id` **não deve ser implementado**. Texto original preservado abaixo apenas como histórico da decisão.



**Decision:** `chats.model_config_id` (nullable) — quando `NULL`, usa o "modelo ativo" marcado globalmente em Conexões (`model_configs.is_active`).
**Reason:** O spec de `connections-models` fala em "modelo ativo" (singular), mas o ROADMAP original (antes desta sessão) já previa "seleção de modelo por chat". O fallback satisfaz os dois sem contradição.
**Trade-off:** Nenhum real — é estritamente mais flexível que só-global.
**Impact:** Fechado no design de `chat-messaging`, não no de `connections-models` (que só define o conceito de "modelo ativo global").

### AD-015: Catálogo de modelos para download é curado, não uma API de catálogo (2026-07-25)

**Decision:** Nem Ollama nem LM Studio expõem API pública para listar "todos os modelos disponíveis para baixar" com tamanho (confirmado via pesquisa web nesta sessão). v1 usa uma lista curada embutida (JSON/const Rust com modelos populares publicamente conhecidos: Llama 3.1 8B, Qwen2.5 7B, Phi-3 mini, etc.) + campo de pull manual por nome (Ollama) ou link Hugging Face (LM Studio) para qualquer coisa fora da lista.
**Reason:** Sem essa decisão, "filtrar modelos para download por memória" (pedido do usuário) seria impossível de implementar de forma alguma — não há de onde vir a lista.
**Trade-off:** A lista curada precisa de manutenção manual ao longo do tempo (novos modelos populares não entram sozinhos); RAM estimada usa fórmula (`params × bytes/peso × 1.2`), rotulada como estimativa na UI, não medição real.
**Impact:** `connections-models/design.md` (`ModelCatalog`) e `tasks.md` T3. Pesquisa confirmou também que LM Studio TEM API de download nativa (`/api/v1/*`, LM Studio ≥0.4.0) — corrige uma suposição errada registrada antes nesta sessão (ver Todos removidos).

### AD-014: Padrão nav+painel para Configurações (2026-07-24)

**Decision:** A seção Configurações na sidebar virou só um item de navegação (ícone + label); os campos (tema, idioma, pasta) saíram do bloco inline da sidebar e passaram a um painel de tela cheia à direita (`SettingsPanel`), substituindo o `ChatPanel` enquanto ativo. Roteamento local via `uiStore` (`activeView: 'chat' | 'settings'`).
**Reason:** Pedido do usuário — a sidebar deve ter "somente a navegação"; os campos aparecem do lado direito ao clicar.
**Trade-off:** Precisa resetar `activeView` para `'chat'` ao criar/selecionar um chat (senão o usuário fica preso na tela de Configurações vendo a lista mudar atrás). Feito em `ChatList.handleCreateChat/handleSelectChat`.
**Impact:** Estabelece o padrão que Documentos e Conexões provavelmente vão seguir quando ganharem conteúdo real (M3/M5) — hoje eles continuam como blocos inline simples (placeholders), a decisão de convertê-los para nav+painel fica para quando tiverem campos de verdade.

### AD-013: Tema "Claude" com a cor oficial da marca (2026-07-24)

**Decision:** Adicionado um 4º tema (`claude`) — paleta creme/terracota usando `#da7756` (confirmado via busca: laranja terracota oficial da Claude/Anthropic) como accent, fundo `#faf9f5`/`#ede9de`.
**Reason:** Pedido explícito do usuário.
**Trade-off:** Só o accent color é confirmado como "oficial"; os tons de fundo creme são uma composição razoável em torno dele, não uma cópia pixel-a-pixel da paleta completa da Anthropic (não tive acesso a um guia de marca oficial completo).
**Impact:** `SUPPORTED_THEMES` agora tem 4 valores; todo `Record<Theme, string>` (Wizard, SettingsPanel) precisa mapear as 4 chaves — TypeScript já força isso via erro de compilação se esquecer.

### AD-012: Config bootstrap fica fora da pasta-base configurável (2026-07-24)

**Decision:** Um arquivo pequeno `config.json` (base_path, theme, language, onboarding_completed) vive no `app_config_dir` padrão do SO (via Tauri), não dentro da pasta-base escolhida pelo usuário. A pasta-base guarda só os dados reais (`localmind.db`, `models/`, `documents/`, `vectors/`, `chats/`).
**Reason:** Ovo-e-galinha: o app precisa saber *onde* está a pasta-base antes de conseguir ler qualquer coisa de dentro dela. Um ponteiro fixo fora da pasta resolve isso e permite trocar a pasta-base livremente depois.
**Trade-off:** Duas localizações de config para o usuário entender (a pasta padrão do SO guarda só o ponteiro; a pasta escolhida guarda os dados). Documentado no README/spec.
**Impact:** `config.rs` implementa `bootstrap_file_path()` separado de `base_path`; `update_base_path` só move o `localmind.db`, nunca o bootstrap.

### AD-011: DbState vira `Mutex<Option<Connection>>` (2026-07-24)

**Decision:** O estado do SQLite no Tauri passou de `Mutex<Connection>` (M1) para `Mutex<Option<Connection>>`, já que agora o banco só existe depois que o usuário completa o wizard (ou quando `update_base_path` remonta a conexão).
**Reason:** Antes do wizard não existe pasta-base, logo não existe onde abrir o `.db`. Comandos de chat agora retornam erro amigável se chamados antes da configuração.
**Trade-off:** Todo comando de chat precisa de um `require_conn`/checagem de `None` a mais.
**Impact:** `commands.rs` (chat) e `config_commands.rs` (onboarding/troca de pasta) compartilham esse padrão; App.tsx só renderiza a Sidebar/ChatPanel depois que `status === "ready"` no configStore, então `list_chats` nunca é chamado com DB ausente na prática.

### AD-001: Framework desktop = Tauri 2 (2026-07-24)

**Decision:** Usar Tauri 2 (Rust + webview) em vez de Electron.
**Reason:** Instalador pequeno (~10-15MB), menor uso de RAM, e backend Rust permite rodar embeddings/banco vetorial nativos.
**Trade-off:** Curva de aprendizado de Rust no backend; menos libs de RAG prontas que no ecossistema JS.
**Impact:** RAG (fastembed, LanceDB) implementado em Rust; frontend em React consome comandos Tauri.

### AD-002: Estratégia de LLM = conectar + runtime embutido (2026-07-24)

**Decision:** Detectar/conectar a Ollama e LM Studio via API OpenAI-compatible E embutir llama.cpp como fallback.
**Reason:** Alinha com "tudo necessário ou comunicação com o necessário"; funciona do zero sem pré-requisitos.
**Trade-off:** Instalador maior e mais complexidade de empacotamento (sidecar por plataforma).
**Impact:** Connection Manager (M2) abstrai runtimes; sidecar llama.cpp isolado em M5 para não travar o MVP.

### AD-003: RAG = embeddings embutidos + vetor local (2026-07-24)

**Decision:** Embeddings com fastembed (ONNX, ex. bge-small) + banco vetorial LanceDB, ambos embutidos.
**Reason:** Indexação 100% offline, sem depender de Ollama estar rodando; nativo em Rust cabe no bundle.
**Trade-off:** Modelo de embedding adiciona ~100-150MB ao bundle.
**Impact:** Ingestão de documentos (M3) independe das conexões de LLM.

### AD-004: Modelo de contexto = chat isolado + docs globais (2026-07-24)

**Decision:** Cada chat é único e isolado (histórico + docs anexados só valem naquele chat). Só a base de documentos é global e compartilhada.
**Reason:** Requisito explícito do usuário — "cada chat é único, somente os documentos são globais".
**Trade-off:** Duas tabelas/namespaces vetoriais (global + por `chat_id`) e lógica de retrieval combinada.
**Impact:** Define o schema (M1) e a arquitetura de RAG em duas camadas (M3 global, M4 por chat).

### AD-010: Config inicial via wizard de primeiro uso, não no instalador (2026-07-24)

**Decision:** Caminho de armazenamento, tema e idioma são definidos por um wizard na 1ª abertura do app (e editáveis depois em Configurações), não durante a instalação.
**Reason:** Instaladores Tauri não suportam config interativa de forma confiável/cross-platform — AppImage (Linux) é portátil sem etapa de instalação, `.deb` instala sem interação; customizar NSIS (Windows) é frágil e inconsistente entre SOs.
**Trade-off:** Configuração acontece 1 clique depois de abrir, não "antes de concluir a instalação" como pedido originalmente.
**Impact:** M2 entrega o wizard de primeiro uso; página customizada no NSIS Windows fica como ideia futura (deferida). Instaladores permanecem padrão/simples em M8.

### AD-009: Memória de conversa = RAG híbrido (recentes verbatim + retrieval do histórico) (2026-07-24)

**Decision:** Contexto de cada mensagem = system prompt + últimas N mensagens verbatim + top-k turnos antigos relevantes (recuperados por embedding) + RAG docs globais + RAG docs do chat/anexos.
**Reason:** Requisito do usuário — conversa serializada funcionando "como memória"; híbrido preserva continuidade imediata E memória de longo prazo além do limite de contexto.
**Trade-off:** Cada turno é embeddado e armazenado num namespace vetorial da conversa (`chat_id`), somando custo/armazenamento.
**Impact:** M6 implementa; reusa o embedding engine do M5. Define 3 camadas de RAG: global (docs), chat (anexos), conversa (memória).

### AD-008: Layout de armazenamento configurável (2026-07-24)

**Decision:** Uma pasta-base escolhida pelo usuário contém `models/`, `documents/`, `vectors/` (LanceDB), `localmind.db` (SQLite) e `chats/<id>/tmp/` para anexos temporários de chat. Anexos de chat são apagados quando o chat é excluído.
**Reason:** Usuário quer escolher onde modelos e documentos ficam; anexos de chat são efêmeros e atrelados ao ciclo de vida do chat.
**Trade-off:** App precisa gerenciar caminhos configuráveis (não só `app_data_dir` do Tauri) e migrar/validar a pasta ao trocar.
**Impact:** M2 define o storage manager e persiste a pasta-base; M4 grava anexos em `chats/<id>/tmp/` e os remove no delete do chat (estende a lógica atual de `delete_chat`).

### AD-007: i18n (EN padrão + PT) e temas múltiplos (2026-07-24)

**Decision:** Interface internacionalizada com inglês como idioma padrão e português disponível; sistema de temas com claro, escuro e temas de cor extras via CSS variables.
**Reason:** Requisito explícito do usuário.
**Trade-off:** Todas as strings de UI precisam passar por camada i18n desde já (retrofit é caro).
**Impact:** M2 introduz i18n (ex.: i18next) e o theme system; textos em PT já escritos na UI do M1 serão movidos para chaves de tradução (EN default).

### AD-006: Tailwind CSS v4 (2026-07-24)

**Decision:** Usar Tailwind CSS v4 (`@tailwindcss/postcss` + `@import "tailwindcss";` no CSS), não v3.
**Reason:** `npm install tailwindcss` instalou a versão atual (4.x) por padrão; v4 não usa `tailwind.config.js`/`@tailwind base/components/utilities` — é config CSS-first com detecção automática de conteúdo.
**Trade-off:** Nenhum, mas qualquer código de exemplo v3 copiado da internet não se aplica diretamente.
**Impact:** `postcss.config.js` usa `@tailwindcss/postcss`; `src/index.css` usa `@import "tailwindcss"` + bloco `@theme`. Não existe `tailwind.config.js` no projeto — é esperado, não um arquivo faltando.

### AD-005: Scaffold via create-tauri-app (2026-07-24)

**Decision:** Projeto gerado com `npx create-tauri-app@latest . -m npm -t react-ts --identifier com.localmind.app -y -f` em vez de escrever tauri.conf.json/Cargo.toml manualmente.
**Reason:** Garante config válida e compatível com a versão atual do Tauri 2 (ícones, capabilities, build.rs corretos).
**Trade-off:** Nenhum.
**Impact:** Estrutura base em `src/`, `src-tauri/`, `package.json` na raiz do projeto.

---

## Active Blockers

_Nenhum._

### B-001 (RESOLVIDO 2026-07-24): Rust toolchain não instalado

**Resolution:** Instalado via `winget install Rustlang.Rustup` (rustc/cargo 1.97.1, toolchain `stable-x86_64-pc-windows-msvc`). MSVC Build Tools já presentes (VS 2019 BuildTools). `cargo check` e `npm run tauri dev` rodaram com sucesso.
**Nota p/ Bash tool:** a PATH do tool Bash não herda o `~/.cargo/bin`; para comandos cargo via Bash, prefixe `export PATH="/c/Users/rafae/.cargo/bin:$PATH"`. Via PowerShell, use `$env:USERPROFILE\.cargo\bin`.

---

## Lessons Learned

### L-001: `create-tauri-app --force` apaga o conteúdo existente do diretório (2026-07-24)

**Context:** Rodei `npx create-tauri-app@latest . -f` dentro de `D:\chat-ia-local`, que já continha `.specs/` com PROJECT.md, ROADMAP.md, STATE.md e o spec do app-shell.
**Problem:** A flag `--force` ("Force create the directory even if it is not empty") apagou a pasta `.specs/` inteira durante o scaffold.
**Solution:** Conteúdo restaurado a partir do histórico da conversa (nenhuma perda real, mas exigiu recriação manual).
**Prevents:** Nunca rodar scaffolders com flag de "force/overwrite" em diretório não vazio sem antes mover/backupar conteúdo existente para fora do diretório alvo, mesmo quando o conteúdo "não deveria" conflitar.

### L-002: M1 verificado em execução (2026-07-24)

**Context:** Após instalar o Rust, `npm run tauri dev` compilou em ~1m34s e abriu a janela (`tauri-app.exe`); `localmind.db` foi criado em `%AppData%\com.localmind.app\`.
**Problem:** Nenhum — validação de que o walking skeleton (Tauri + React + SQLite) funciona ponta a ponta.
**Solution:** SHELL-08 (init DB + migrações) confirmado pela criação do .db. Verificação visual dos fluxos de CRUD de chat (SHELL-01..07) ainda depende de clicar na UI manualmente.
**Prevents:** Regressões futuras — temos baseline de que o backend Rust compila e o app sobe nesta máquina.

---

## Quick Tasks Completed

| #   | Description | Date | Commit | Status |
| --- | ----------- | ---- | ------ | ------ |
| —   | —           | —    | —      | —      |

---

## Deferred Ideas

- [ ] Perfis de agente reutilizáveis (persona + modelo + docs) — Captured during: planejamento inicial
- [ ] Agentes com ferramentas / tool-calling — Captured during: planejamento inicial
- [ ] Suporte a macOS — Captured during: planejamento inicial
- [ ] OCR de documentos escaneados — Captured during: planejamento inicial
- [ ] Página customizada no instalador NSIS Windows (pasta de dados durante a instalação) — Captured during: replanejamento (ver AD-010); wizard de 1º uso cobre isso no v1
- [ ] Detecção de VRAM por GPU para filtragem de modelos mais precisa — Captured during: replanejamento (M3 começa só com RAM)
- [ ] Build CUDA do llama.cpp embutido, para quem tem NVIDIA de ponta (~35% mais rápido em prompt processing que Vulkan) — Captured during: design do M7 (ver AD-022)
- [ ] Atualizar o binário do llama.cpp embutido para releases mais novos (v1 fixa o tag resolvido no primeiro download) — Captured during: spec do M7

---

## Todos

- [ ] **Lacuna do M2 encontrada na auditoria de 2026-07-25**: se a pasta-base persistida sumiu/foi movida entre sessões, `lib.rs` só faz `eprintln!` e deixa o `DbState` como `None`; o `configStore` vê `onboarding_completed: true` e entra em `ready`, então o app abre e **todo** comando falha com "Nenhuma pasta de armazenamento configurada ainda". O spec do `settings-storage-i18n` manda avisar e reabrir o wizard — não implementado
- [ ] Verificar manualmente na UI os fluxos de CRUD de chat do M1 (criar/renomear/excluir/persistir após reiniciar) — SHELL-01..07
- [ ] Verificar `connections-models` (M3) com Ollama e/ou LM Studio rodando de verdade nesta máquina — implementado e com `tauri dev` subindo limpo, mas `OllamaClient`/`LmStudioClient`/download real/`configure_model` nunca foram exercitados contra um servidor real (nenhum estava rodando durante a execução) — ver AD-019
- [x] ~~**1º — Executar `single-active-connection` tasks.md** (10 tasks)~~ — feito em 2026-07-25 (ver AD-023)
- [ ] Verificar manualmente na UI o fluxo do par ativo: ativar Ollama → ativar LM Studio → só a última marcada; escolher modelo da outra conexão → conexão ativa acompanha (T9 do `single-active-connection`, único item não verificado)
- [x] ~~**2º — Executar `embedded-runtime` tasks.md** (16 tasks)~~ — feito em 2026-07-25 (ver AD-024); URL do Phi-3.5 verificada ao vivo, C-07/C-02 pagos
- [ ] Verificar na UI o fluxo do runtime embutido: clicar em instalar no card (baixa ~2,4 GB do Phi-3.5), ativar a conexão embutida, fechar o app e confirmar no `tasklist` que o `llama-server` sumiu, reabrir e confirmar o autostart (T16, EMBED-06/07)
- [x] ~~**3º — Executar `documents-rag` tasks.md** (11 tasks)~~ — feito em 2026-07-25 (ver AD-025)
- [x] ~~**4º — Executar `chat-messaging` tasks.md** (12 tasks)~~ — feito em 2026-07-25 (ver AD-026)
- [x] ~~Pesquisa obrigatória de crates/modelos em `documents-rag` T3/T4/T5~~ — feita e registrada na AD-025
- [ ] **Verificar na UI o fluxo do M5/M4**: importar um documento e ver chegar a "ready"; enviar mensagem e ver streaming; anexar um `.txt` com um fato inventado e perguntar sobre ele; repetir a pergunta em outro chat e confirmar que NÃO usa o contexto (CHAT-11); excluir o chat e confirmar que `chats/<id>/tmp/` sumiu (CHAT-12)
- [ ] **Pré-requisito de build novo**: `protoc` (instalado via winget nesta máquina) é obrigatório para compilar o `lancedb` — documentar no README/STACK antes de qualquer outra pessoa clonar o repo
- [ ] O `onnxruntime.dll` é baixado em runtime na primeira indexação (~79 MB); nunca foi exercitado pelo caminho do app (só por teste com a DLL apontada à mão) — confirmar que `rag::onnxruntime::ensure_dylib` baixa e extrai certo
- [ ] Encarar os itens de `.specs/codebase/CONCERNS.md` não cobertos pelas features planejadas: C-03 (espelhamento manual de tipos Rust↔TS), C-04 (zero teste no frontend), C-06 (polling de download do LM Studio sem timeout), C-09 (sem linter/CI), C-10, C-11
- [ ] Avaliar assinatura de código dos instaladores (Windows) — design M8
- [ ] Depois do M1, avaliar excluir os ícones padrão do template (`Square*.png`, `StoreLogo.png`) não usados no bundle final

---

## Preferences

**Model Guidance Shown:** never

# LocalMind — Chat de IA Local com RAG

**Vision:** Aplicação desktop offline-first que funciona como um chat de IA com o modelo rodando na própria máquina, com uma base de conhecimento em documentos usada como RAG — empacotada em um instalador único para Windows e Linux que já traz o motor dentro dele.
**For:** Usuários técnicos e knowledge workers que querem conversar com uma IA usando seus próprios documentos, 100% local, sem enviar dados para a nuvem.
**Solves:** Ferramentas de chat com IA hoje dependem da nuvem (privacidade/custo) ou exigem montar manualmente um pipeline de RAG local. LocalMind entrega isso pronto, em um único instalador, sem configuração de servidores.

## Goals

- **Privacidade total:** nenhum dado (conversas ou documentos) sai da máquina do usuário — 100% offline por padrão.
- **Zero-setup:** instalar e usar. O runtime (llama.cpp), o motor de embeddings e o leitor de PDF viajam dentro do instalador; da internet só é preciso o download de **um modelo GGUF** escolhido pelo usuário (AD-039/M9).
- **RAG em duas camadas:** documentos globais (base de conhecimento) buscáveis por qualquer chat + documentos anexados por chat, com contexto isolado naquele chat.
- **Instalador único multiplataforma:** um artefato por SO (Windows `.msi`/`.exe`, Linux `.AppImage`/`.deb`) contendo tudo necessário.

## Tech Stack

**Core:**

- Framework desktop: **Tauri 2.x** (webview nativo do SO + backend Rust)
- Frontend: **React 18 + TypeScript + Vite**, estilização com **Tailwind CSS**, estado com **Zustand**
- Backend: **Rust** (comandos Tauri, async via Tokio)
- Persistência de metadados: **SQLite** (chats, mensagens, runtime ativo, metadados de documentos)

**Key dependencies:**

- **LanceDB** (banco vetorial embutido, nativo em Rust) — armazena embeddings
- **fastembed-rs** (embeddings ONNX embutidos, ex.: `bge-small`/`all-MiniLM`) — indexa docs 100% offline
- **llama.cpp** (`llama-server` como sidecar) — o único runtime LLM, empacotado no instalador nas variantes Vulkan e CPU
- **pdfium** e **ONNX Runtime**, também empacotados — nada de componente binário baixado em tempo de execução
- Parsers de documento (PDF, DOCX, TXT, MD) em Rust
- API **OpenAI-compatible** internamente, que é o protocolo que o `llama-server` fala

## Scope

**v1 includes:**

- Janela desktop com sidebar de 3 zonas: **Chats** (topo), **Documentos/base de conhecimento** (meio), **Runtime** (base)
- Gerenciamento de chats (criar, listar, renomear, excluir) com histórico persistente e isolado por chat
- Um runtime embutido (llama.cpp), com escolha automática entre os backends Vulkan e CPU, lista de modelos instalados e catálogo para baixar
- Chat com streaming e system prompt opcional
- Importar documentos para a base global → parse, chunking, embedding, busca por similaridade (RAG)
- Anexar documentos dentro de um chat (contexto isolado ao chat); docs pequenos injetados inteiros, grandes via RAG
- Instaladores para Windows e Linux via CI

**Explicitly out of scope (v1):**

- Perfis de agente reutilizáveis (persona + modelo + docs) e agentes com ferramentas/tool-calling → roadmap futuro
- Sincronização em nuvem, multiusuário ou colaboração
- Suporte a macOS (foco Windows/Linux no v1)
- Fine-tuning ou treino de modelos
- OCR de imagens/documentos escaneados

## Constraints

- **Técnico:** offline-first obrigatório; nenhuma chamada de rede externa por padrão. Instalador único e autossuficiente por SO.
- **Recursos:** embeddings e vetores rodam nativos em Rust para caber no bundle sem dependências pesadas de Python.
- **Compatibilidade:** um runtime só, embutido. Falar com um servidor OpenAI-compatible externo (vLLM, TGI, um Ollama que a pessoa já tenha) foi removido de propósito na AD-039 — se voltar, volta como feature nova, não como resíduo de arquitetura.

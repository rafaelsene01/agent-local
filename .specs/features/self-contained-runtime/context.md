# Runtime autossuficiente — Decisões do usuário

**Data:** 2026-07-26
**Origem:** *"acho que pode remover a integração com ollama e lmstudio, runtime embutido creio que deve ser o suficiente, quero que o programa seja auto suficiente, não precisando de outros programa para rodar"*

Três perguntas diretas fecharam as áreas cinzentas antes de escrever a spec. As respostas abaixo são decisões travadas — o design não as reabre.

---

## D1 — A conexão "custom" (URL manual OpenAI-compatible) também sai

**Escolha:** remover também.

**O que isso significa na prática (foi exatamente o que a opção descrevia):**
- Sobra **um único runtime**: o embutido (llama.cpp).
- Some a tela de Conexões e o formulário de adicionar URL.
- A tabela `connections` deixa de ter razão de existir — não há o que escolher entre.
- O trait `ProviderClient` (hoje com 4 implementações e `Box<dyn>`) colapsa num cliente concreto só.

**O que se perde, conscientemente:** o escape hatch para quem já tem um servidor OpenAI-compatible rodando (vLLM, TGI, um Ollama existente). Se algum dia voltar, volta como feature nova e explícita, não como resíduo de arquitetura.

---

## D2 — Os componentes binários passam a ir dentro do instalador

**Escolha:** embutir no instalador.

Hoje o app baixa três coisas da internet no primeiro uso:

| Componente | De onde | Tamanho medido |
| --- | --- | --- |
| `llama-server` (Vulkan) | release do `ggml-org/llama.cpp` | 33,5 MB (zip, `b10142`) |
| `llama-server` (CPU, fallback) | mesmo release | 18,3 MB (zip) |
| ONNX Runtime | release do `microsoft/onnxruntime` | ~79 MB (zip completo) |
| pdfium | `bblanchon/pdfium-binaries` | 3,74 MB |

Passam todos a viajar dentro do instalador. Consequências aceitas:
- Instalador cresce (~120–200 MB por SO, a **medir** depois do primeiro build).
- O CI passa a baixar e empacotar esses artefatos em toda release.
- Atualizar a versão do llama.cpp/ONNX/pdfium passa a exigir uma release nova do LocalMind — deixa de ser "o app pega a última".
- Em troca: **zero download de componente**, inclusive numa máquina sem internet nenhuma.

---

## D3 — O modelo LLM continua sendo baixado

**Escolha:** continuar baixando do catálogo.

O usuário escolhe o GGUF que cabe na máquina dele (catálogo de 6 modelos, 986 MB a 4,9 GB). O instalador não carrega modelo.

**Consequência honesta, que a spec precisa afirmar sem maquiagem:** o app fica autossuficiente em **programas** (não depende de mais nada instalado) e em **componentes** (não baixa binário nenhum), mas ainda precisa de internet **uma vez** para trazer um modelo. Depois disso, funciona offline para sempre.

---

## Não perguntado, decidido por julgamento

| Questão | Decisão | Por quê |
| --- | --- | --- |
| Bancos com Ollama/LM Studio já configurados | Migração apaga as conexões externas; chats, mensagens e documentos ficam intactos | É a consequência direta de D1; preservar linhas de um provedor que o código não sabe mais falar seria lixo com aparência de dado |
| Restos de download de versões anteriores (`<base>/runtime/{vulkan,cpu,onnxruntime,pdfium}`) | Apagados no boot | São ~150 MB que nunca mais serão lidos |
| Ambos os backends (Vulkan **e** CPU) embutidos | Sim | O fallback de CPU existe para máquinas onde o binário Vulkan nem executa. Embutir só o Vulkan trocaria "baixa 18 MB" por "o app não funciona nessa máquina" |
| `model_configs` | Sai junto com `connections` | Com um runtime só, a linha `embedded_runtime` já guarda modelo, contexto e GPU — manter os dois lugares foi exatamente o que causou o EMBED-12 (config gravada e ignorada) |

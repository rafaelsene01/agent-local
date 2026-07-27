# Memória de conversa — contexto e decisões do usuário

Perguntas feitas antes de escrever a spec, com as respostas dadas em 2026-07-27. Estão aqui porque
cada uma muda o desenho, não só o texto.

---

## 1. Controle: toggle por conversa, e memória confinada à conversa

**Resposta:** *"pode ter um toggle, mas a memoria do chat deve ser restrita ao chat daquela conversa"*.

Duas coisas numa frase, e a segunda é a mais forte:

- **Toggle** — o mesmo padrão que `chats.use_global_rag` já usa. Uma coluna, um controle ao lado do
  que já existe. Ligado por padrão, porque memória desligada é o comportamento de hoje e ninguém
  precisaria da feature para tê-lo.
- **Confinamento** — a memória de um chat **não é recuperável por outro chat**, nunca. Isso vira o
  MEM-07, um requisito de isolamento com teste próprio, e não uma consequência esperada do
  namespace. A distinção importa: a CHAT-11 já foi um isolamento que *parecia* garantido pelo
  namespace e estava furado (AD-040) — a linha temporária de um anexo era reprocessada no boot com
  o namespace global. O jeito de não repetir isso é afirmar o isolamento como requisito e testá-lo.

## 2. Histórico existente: backfill sob demanda, por conversa

**Resposta:** backfill sob demanda, por chat.

Turnos novos entram na memória quando acontecem. O histórico já gravado só entra se o usuário pedir,
por conversa. Descartadas:

- *só daqui pra frente* — deixaria as conversas atuais permanentemente sem memória, sem saída;
- *backfill automático no boot* — numa base grande é CPU de embedding no primeiro boot depois do
  update, exatamente o que o usuário lê como travamento.

**Consequência de desenho:** como existe um caminho explícito para reindexar, o toggle desligado pode
parar **de gravar** também, não só de recuperar — religar não é um beco sem saída, é um botão.

## 3. Unidade da memória: o par pergunta + resposta

**Resposta:** o par pergunta+resposta.

Um turno completo vira uma unidade de memória. Uma resposta isolada ("sim, exatamente") não
significa nada fora da pergunta que a originou, e recuperá-la sozinha injetaria ruído no prompt do
mesmo jeito que os chunks corrompidos da AD-033 injetavam.

---

## O que já estava decidido antes destas perguntas

**AD-009 (2026-07-24):** contexto de cada mensagem = system prompt + últimas N verbatim + top-k
turnos antigos relevantes + RAG global + RAG de anexos. Três camadas, e esta feature entrega a
terceira.

**AD-033 (2026-07-26):** o histórico recente fica colado na pergunta e o modelo responde ao que está
perto dela. Essa medição é o que faz a memória entrar **depois** do histórico recente no orçamento,
nunca no lugar dele — a camada nova não pode empurrar para longe o que já se provou ser o que o
modelo lê.

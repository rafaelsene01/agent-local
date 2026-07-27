# LocalMind

Um chat de IA que roda inteiro na sua máquina, com os seus documentos como base de conhecimento.

Sem conta, sem servidor, sem assinatura. As conversas ficam num banco SQLite na pasta que você escolher, os documentos são indexados localmente, e o modelo de linguagem roda como um processo filho do próprio app.

---

## O problema

Conversar com uma IA sobre documentos próprios hoje custa uma de duas coisas: mandar os arquivos para a nuvem, ou montar um pipeline de RAG na mão — servidor de modelo, banco vetorial, extrator de PDF, gerador de embeddings, cada um com sua instalação e sua versão.

O LocalMind entrega isso pronto num instalador. Você abre, escolhe onde guardar seus dados, e conversa.

## O que ele faz

**Conversa com streaming.** A resposta aparece token a token e pode ser cancelada no meio. O que já apareceu na tela fica salvo — cancelar não apaga.

**Base de conhecimento.** Importe PDF, DOCX, TXT ou MD. Cada arquivo passa por extração, divisão em trechos e geração de embeddings, com o progresso visível; só entra na busca depois de pronto. Nas respostas, os trechos usados vêm citados pelo nome do arquivo, para você poder conferir.

**Anexos por conversa.** Arquivos enviados dentro de um chat valem só para aquele chat — outra conversa não os enxerga. Arquivos pequenos entram inteiros no contexto; grandes são indexados. Apagar o chat apaga os arquivos e os vetores junto.

**Modelo rodando localmente.** O app baixa o `llama.cpp` e o modelo que você escolher, e cuida do processo: inicia junto com o app, encerra junto, e usa a GPU quando existe uma — Vulkan cobre NVIDIA, AMD e Intel sem instalar toolkit nenhum. O catálogo mostra o tamanho real de cada download e avisa o que não cabe na sua memória.

**Ajuste fino.** Tamanho de contexto com o teto real do modelo (lido do arquivo, não chutado) e escolha entre CPU e GPU.

**Seus dados, na sua pasta.** Você define onde ficam banco, modelos, documentos e vetores — inclusive num drive externo. Há uma versão portátil que guarda tudo ao lado do executável e não escreve nada em `%APPDATA%` nem no registro do Windows.

**Interface em dois idiomas** (português e inglês) e quatro temas.

**Atualização automática**, que avisa quando há versão nova e pode ser desligada.

## Sobre privacidade, sem exagero

Conversas e documentos nunca saem da máquina. Nenhum texto seu é enviado para lugar nenhum — o modelo responde localmente.

O que **usa rede**, para ser justo com você:

- o download inicial do runtime, do modelo e das bibliotecas de extração e embeddings;
- a verificação de atualização, que roda depois do primeiro uso e tem um botão para desligar.

Feito isso, o app funciona sem internet.

## Baixar

Na [página de Releases](https://github.com/rafaelsene01/local-mind/releases) há uma opção para cada caso:

| Arquivo | Para quem |
| --- | --- |
| `-setup.exe` | Windows. Instala na sua conta de usuário — **sem pedir administrador** |
| `_x64_en-US.msi` | Windows, para instalação gerenciada |
| `-portable.zip` | Windows sem permissão de instalar nada. Extraia e execute; roda de pendrive |
| `.AppImage` | Linux. Dê permissão de execução e rode, sem instalar |
| `.deb` | Debian, Ubuntu e derivados |

Todos os pacotes são assinados, e o app verifica a assinatura antes de aplicar qualquer atualização.

> O executável ainda não tem certificado de assinatura de código, então o SmartScreen do Windows avisa na primeira execução. É esperado.

## Estado do projeto

Primeira versão publicada: **v0.1.1**. O que está pronto e em uso: chat com streaming, base de conhecimento com RAG e citações, anexos por conversa, runtime local com GPU, temas e idiomas, pacotes para Windows e Linux, e atualização automática.

Em andamento no branch principal: o app passou a ter **um único runtime, embutido no instalador**. O motor (llama.cpp, nas variantes GPU e CPU), o mecanismo de embeddings e o leitor de PDF agora viajam dentro do pacote — não há mais nada para baixar depois de instalar, exceto o modelo que você escolher. As versões iniciais se conectavam a programas externos; esse suporte saiu.

Essa mudança ainda **não foi publicada nem exercitada numa instalação real** — as versões lançadas continuam sendo as anteriores. Prefira uma versão publicada.

Também em andamento no branch principal: **memória de conversas longas**. Cada pergunta respondida vira um trecho recuperável da própria conversa, então o chat volta a encontrar o que foi dito muito antes, mesmo fora da janela de contexto. Fica restrito à conversa — um chat nunca lembra do outro —, pode ser desligado por conversa, e há um botão para indexar o histórico que você já tem. Como o resto do branch, **ainda não foi exercitado numa instalação real**.

Ainda não existe: macOS.

## Para quem quiser olhar por dentro

O projeto é desenvolvido por specs: cada funcionalidade tem requisitos rastreáveis, decisões registradas com o motivo, e o que foi verificado de verdade separado do que só compilou. Tudo isso está em [`.specs/`](.specs/project/PROJECT.md) — começando pela visão e pelo [roadmap](.specs/project/ROADMAP.md), com o histórico de decisões em [STATE.md](.specs/project/STATE.md).

O processo de publicação de uma versão está em [docs/RELEASING.md](docs/RELEASING.md).

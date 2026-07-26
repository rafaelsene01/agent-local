# Release & Distribution — Context

**Gathered:** 2026-07-26
**Spec:** `.specs/features/release-distribution/spec.md`
**Status:** Ready for design

---

## Feature Boundary

Entregar o M8 completo: pipeline de CI/CD no GitHub Actions que **valida** todo push/PR e **publica releases apenas por disparo manual**, com versionamento semântico e CHANGELOG gerado dos commits; toda release carrega os artefatos de instalação (`.msi`, `-setup.exe`, `.deb`, `.AppImage`) **mais um bundle portátil**; e o app passa a verificar sozinho se há versão nova, perguntar ao usuário e se atualizar — inclusive no modo portátil, **sem nunca pedir credenciais de administrador**.

---

## Implementation Decisions

### Modelo de branches

- **`master` + feature branches.** Sem `develop`, sem `release/*`. Projeto solo, um mantenedor — uma branch de integração separada só adicionaria um merge sem ganho.
- Releases saem **sempre de `master`**. O workflow recusa disparo a partir de qualquer outra ref.
- Feature branches → PR → merge em `master`. O CI de validação roda em push e em PR.

### Como a versão é decidida

- O disparo é **`workflow_dispatch` com um select `bump` = `major` | `minor` | `patch`** — o usuário escolhe, o CI não adivinha.
- Escolhido o bump, **o CI faz o resto sozinho na mesma execução**: calcula a nova versão a partir da última tag, grava essa versão nos arquivos que precisam dela, gera o CHANGELOG a partir dos commits desde a última tag, cria o commit de release, cria a tag e publica a GitHub Release com esse conteúdo.
- Nenhuma versão é digitada à mão em lugar nenhum. O único input humano é o bump.
- Conventional Commits continuam sendo a convenção — não para *decidir* a versão (isso é o select), mas para o CHANGELOG sair agrupado e legível.

### Artefatos e estratégia de atualização

- Toda release traz **os instaladores nativos** — `.msi` e `-setup.exe` (Windows), `.deb` e `.AppImage` (Linux) — **e um `.zip` portátil**.
- **Dois caminhos de atualização, uma única UI:**
  - **Instalado** → `tauri-plugin-updater` oficial (baixa o instalador, valida assinatura, instala, reinicia).
  - **Portátil** → atualizador próprio: baixa o `.zip`, valida a mesma assinatura minisign, troca os arquivos no lugar e relança. Zero elevação de privilégio.
- O NSIS fica em `installMode: currentUser` (instala em `%LOCALAPPDATA%`, não pede admin) — é o padrão do Tauri e vai ficar explícito na config para não regredir.
- O app descobre em que modo está por um **arquivo marcador** dentro do bundle portátil, não por heurística de caminho.

### Comportamento do app ao encontrar versão nova

- **Verifica no boot** (silenciosamente, alguns segundos depois de abrir e só depois do onboarding concluído).
- Achou versão nova → **banner não bloqueante** com número da versão, notas e três ações: **Atualizar**, **Depois**, **Pular esta versão**.
- Download com **progresso visível**; ao terminar, reinicia sozinho.
- **Configurações ganha uma seção "Atualizações"** com: versão atual instalada, botão **"Verificar agora"** e um **toggle para desligar a verificação automática** (opt-out). O toggle é a forma de preservar o "offline-first" do PROJECT.md como escolha explícita do usuário, não como padrão silencioso.
- Padrão do toggle: **ligado**.

### Agent's Discretion

- Ferramenta de geração de CHANGELOG, formato do `cliff.toml`/config equivalente e agrupamento das seções.
- Layout visual exato do banner e da seção de Configurações (deve seguir os padrões já usados em `SettingsPanel`/`DocumentsPanel`).
- Nomes de arquivos, módulos Rust e comandos Tauri.
- Estrutura interna do `.zip` portátil e nome do arquivo marcador.
- Divisão dos jobs do workflow e estratégia de cache.

---

## Specific References

- *"toda nova release gerado quero que deixe os arquivos de instalação"* — os instaladores nativos são obrigatórios em toda release, não opcionais.
- *"como se fosse algo portatil, pois pode ter computador que não deixa instalar, pedindo credenciais de administrador"* — o critério de sucesso do modo portátil é **nunca disparar UAC**, nem na primeira execução nem na atualização.
- *"nem que o bundle disponivel na release, tenha algo extra para isso além dos arquivos que pode ser usado para primeira instalação"* — o usuário autorizou explicitamente um artefato **adicional** na release para viabilizar o auto-update portátil.
- *"na execução ele deve gerar changelog, criar a tag e atualizar a versão da release com base no que eu escolhi"* — as quatro coisas (versão, CHANGELOG, tag, release) acontecem em **uma única execução** do workflow.

---

## Deferred Ideas

Levantados durante a análise, **fora do escopo** desta feature:

- **Code signing de verdade** (certificado Authenticode no Windows, notarização macOS). Sem certificado, o SmartScreen vai avisar na primeira execução. É custo/burocracia externa, não código.
- **Canal beta / pré-releases** (`0.4.0-beta.1`) para testar o auto-update antes de soltar estável — foi oferecido como modelo de branches e recusado em favor da simplicidade.
- **macOS** — já é Future Consideration no ROADMAP; o pipeline não vai ter runner `macos-*`.
- **Delta updates** (baixar só o diff em vez do bundle inteiro).
- **`cargo clippy -D warnings` e `cargo fmt --check` no CI** — o código atual não passa nesses gates hoje (há dead code conhecido, ver AD-033); adicionar agora transformaria a introdução do CI numa refatoração.
- **Rollback para versão anterior pela UI.**

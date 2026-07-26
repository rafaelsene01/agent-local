# Publicando uma release

Releases do LocalMind são **manuais por definição**. O workflow `release.yml` tem
um único gatilho — `workflow_dispatch` — e nenhum `push`, `tag` ou `schedule`.
Um merge em `master` nunca publica nada.

---

## Setup (uma vez só)

O pipeline assina todos os artefatos com uma chave minisign. Sem ela, o build
até roda, mas nenhuma atualização automática funciona — o app recusa qualquer
pacote que não valide contra a chave pública.

```bash
# 1. Gerar o par de chaves. Guarde a senha num gerenciador: sem ela,
#    ou sem o arquivo, não há como assinar novas versões — e um app já
#    instalado nunca mais aceitará uma atualização sua.
npx tauri signer generate -w ~/.tauri/localmind.key

# 2. Cadastrar os segredos no repositório
gh secret set TAURI_SIGNING_PRIVATE_KEY < ~/.tauri/localmind.key
gh secret set TAURI_SIGNING_PRIVATE_KEY_PASSWORD

# 3. Copiar a chave PÚBLICA (~/.tauri/localmind.key.pub) para
#    src-tauri/tauri.conf.json -> plugins.updater.pubkey  e commitar.
#    A pública vai para o repositório; a privada nunca.
```

Confira com `gh secret list` — os dois segredos precisam aparecer.

> **A chave privada não entra no repositório.** O arquivo em
> `src-tauri/tests/fixtures/` é um par descartável, gerado só para os testes de
> verificação de assinatura, e não assina nada que seja distribuído.

### Rotacionar a chave

Trocar a chave **quebra a atualização automática de quem já está instalado**:
o app instalado só confia na chave pública que veio embutida nele. Depois de
rotacionar, quem estiver numa versão antiga precisa baixar a nova à mão uma
única vez. Rotacione apenas se a chave privada vazar.

---

## Publicar

1. Confirme que o CI está verde em `master`.
2. **Actions → Release → Run workflow**.
3. Escolha o `bump`:

| Bump | Quando | `0.4.2` vira |
| --- | --- | --- |
| `patch` | só correções, nada mudou para o usuário | `0.4.3` |
| `minor` | funcionalidade nova, compatível | `0.5.0` |
| `major` | quebra algo (formato de dados, migração sem volta, remoção de recurso) | `1.0.0` |

O resto é automático, numa única execução:

- calcula a versão a partir da última tag `v*` (na primeira vez, a partir do `package.json`);
- grava a versão em `package.json`, `package-lock.json`, `Cargo.toml` e `Cargo.lock` (o `tauri.conf.json` não entra na lista: o campo `version` dele é `"../package.json"`, então o Tauri lê a versão de lá na hora do build);
- gera o `CHANGELOG.md` a partir dos Conventional Commits desde a última tag;
- commita `chore(release): vX.Y.Z`, cria a tag e faz o push;
- compila e empacota no Windows e no Linux, assinando tudo;
- monta e assina o `.zip` portátil;
- acrescenta a entrada portátil no `latest.json`;
- **só então** tira a release do rascunho.

Ao terminar, a release deve conter:

```
LocalMind_X.Y.Z_x64_en-US.msi          (+ .sig)
LocalMind_X.Y.Z_x64-setup.exe          (+ .sig)
LocalMind_X.Y.Z_amd64.deb
LocalMind_X.Y.Z_amd64.AppImage         (+ .sig)
LocalMind_X.Y.Z_x64-portable.zip       (+ .sig)
latest.json
```

O `latest.json` precisa ter as chaves `windows-x86_64-nsis`,
`windows-x86_64-msi`, `linux-x86_64-appimage` **e** `windows-x86_64-portable`.
Se a última faltar, o auto-update do modo portátil simplesmente não encontra
nada — e não reclama, por desenho.

---

## Quando o workflow falha no meio

As guardas (branch errada, tag já existente) rodam **antes** de qualquer
escrita, então uma falha ali não deixa resíduo: corrija e rode de novo.

Depois que a tag foi empurrada, o repositório já mudou. Para repetir:

```bash
TAG=vX.Y.Z
gh release delete "$TAG" --yes          # a release em rascunho
git push --delete origin "$TAG"         # a tag remota
git tag -d "$TAG"                       # a tag local
git revert --no-edit <sha-do-chore-release>   # ou reset, se ninguém puxou ainda
```

Depois dispare o workflow de novo com o mesmo bump.

**Um SO falhou e o outro passou:** a release fica em rascunho com os artefatos
do que passou. Não publique pela metade — o `latest.json` estaria incompleto e o
app ofereceria uma atualização que não existe para parte dos usuários. Apague e
rode de novo.

---

## Convenção de commits

O CHANGELOG é gerado das mensagens de commit, então elas são parte do produto:

```
feat(chat): ...     → Novidades
fix(rag): ...       → Correções
perf, refactor, docs, test, build, ci, chore  → seções próprias
BREAKING CHANGE no corpo  → Mudanças incompatíveis
```

O CI valida isso nos PRs. Commits direto em `master` **não** passam por essa
validação — mensagem fora do padrão cai em "Outros" no changelog.

---

## Sem code signing

Os instaladores **não** são assinados com certificado Authenticode. Na primeira
execução, o SmartScreen do Windows vai mostrar "Windows protegeu o computador" e
exigir *Mais informações → Executar assim mesmo*. Isso é esperado e não indica
problema — assinar de verdade exige comprar um certificado, e está registrado
como ideia diferida no `STATE.md`.

A assinatura minisign descrita acima é outra coisa: ela protege o **canal de
atualização** (o app só instala pacotes assinados pela chave do projeto), não a
reputação do executável perante o Windows.

---

## Modo portátil

O `.zip` existe para máquinas onde instalar exige administrador. Ele contém:

```
LocalMind/
├── LocalMind.exe
├── .portable      ← não apague: é o que mantém o modo portátil
└── README.txt
```

O app detecta o marcador e passa a gravar tudo em `./data`, ao lado do
executável — nada em `%APPDATA%`, nada no registro. A atualização renomeia o
executável em uso, move o novo no lugar e relança, sem elevação. Se a pasta não
permitir escrita (pendrive protegido, `Program Files`), o app avisa **antes** de
baixar.

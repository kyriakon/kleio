# Agent setup guide (omp + skills + extensions)

How to stand up this repo's agentic workflow on a new machine — or onboard a new contributor. Reference setup in use by Oliver; install steps are the contract.

## 1. Install the harness

- [omp](https://github.com/can1357/oh-my-pi) via Homebrew: `brew install omp`
- [Bun](https://bun.sh) — required for extension loading: `curl -fsSL https://bun.sh/install | bash`

## 2. Model roles

omp routes work through five roles (`~/.omp/agent/config.yml` → `modelRoles`):

| Role | Online (DeepSeek cloud) | Offline (local MLX) |
|---|---|---|
| `default` | `deepseek/deepseek-v4-flash` | `mlx-main/...Qwen2.5-Coder-7B` |
| `smol` | `mlx-smol/...MiniCPM5-1B` | same |
| `commit` | `mlx-smol/...MiniCPM5-1B` | same |
| `slow` | `deepseek/deepseek-v4-pro` | Qwen 7B |
| `plan` | `deepseek/deepseek-v4-pro` | Qwen 7B |

- `default` is the everyday model; `slow`/`plan` are for deep reasoning (wayfinder grilling, ADR drafting, code review). DeepSeek's supported ids are `deepseek-v4-pro` and `deepseek-v4-flash` — probe with the chat-completions endpoint; `/v1/models` is gated.
- **Secrets rule: API keys live in the environment only** — `export DEEPSEEK_API_KEY=...` in `~/.zshrc` (or `~/.omp/agent/.env`). omp resolves provider keys from env natively (`getEnvApiKey`). Never put a key in `models.yml`, `config.yml`, or any committed file. `models.template.yml` must stay key-free; the launch script only copies it.

### Online/offline swap

`~/.bin/start-omp.sh` (aliased as `omp` in `~/.zshrc`) is the launcher: it offers online (DeepSeek + local smol) or offline (local only) mode, copies `config.online.yml` / `config.offline.yml` → `config.yml`, copies `models.template.yml` → `models.yml`, starts the MLX servers (`mlx_lm.server`, ports 11435/11436), then runs omp. Extensions/skills are unchanged between modes.

## 3. Extensions (marketplace)

Both personal extensions install from the marketplace repo — no manual file placement:

```sh
omp plugin marketplace add oliverbrotchie/omp-extensions
omp plugin install ponytail@omp-extensions caveman@omp-extensions
# after upstream/catalog changes:
omp plugin upgrade ponytail@omp-extensions caveman@omp-extensions
```

- **ponytail** — lazy-senior-dev mode; sourced from upstream [DietrichGebert/ponytail](https://github.com/DietrichGebert/ponytail) (the catalog points at it, nothing vendored).
- **caveman** — terse-prose mode; vendored in the marketplace repo (`plugins/caveman/`).

Marketplace layout: `.omp-plugin/marketplace.json` catalog; a plugin ships `package.json` with `"omp": { "extensions": [...] }` for extension modules, or `skills/` for skills. Extensions are TS/JS loaded with Bun; sources must stay in the marketplace repo, never loose files in `~/.omp/agent/extensions/`.

## 4. Skills

The matt-pocock skill set (ask-matt, wayfinder, grilling, domain-modeling, code-review, …) installs from its marketplace:

```sh
omp plugin marketplace add mattpocock/skills
# install the skills you need; the skill set is listed in ~/.agents/skills/
```

## 5. Repo conventions

Once omp is running in the kleio repo, the workflow is documented in:

- `AGENTS.md` — build/test commands, code style, tickets, git rules
- `docs/agents/issue-tracker.md` — tracker ops incl. the wayfinder close-checklist
- `docs/agents/domain.md` — where vocabulary/ADRs live (`CONTEXT.md`, `docs/decisions/`)
- `docs/decisions/` — ADRs (read before assuming missing context)

The repo's pre-commit hook (betterleaks + cargo check) runs on every commit — no extra agent-side setup needed.

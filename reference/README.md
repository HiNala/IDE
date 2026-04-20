# Reference PRDs (source of truth)

These files are **verbatim product specification** material. They define *what* we
are building and *why*. They are not implementation playbooks — implementation
notes live under `docs/`.

| File | Summary |
|------|---------|
| [`00_PRODUCT_REQUIREMENTS.md`](./00_PRODUCT_REQUIREMENTS.md) | MVP vision, constraints, subsystem summary. |
| [`01_TECH_STACK.md`](./01_TECH_STACK.md) | Technology choices and version policy (mirrors `docs/TECH_STACK.md`). |
| [`02_ARCHITECTURE_STRATEGY.md`](./02_ARCHITECTURE_STRATEGY.md) | Architecture + performance model (combined view). |
| [`03_GAPS_AND_RISKS.md`](./03_GAPS_AND_RISKS.md) | Risk register and mitigations. |
| [`04_MVP_DEFINITION.md`](./04_MVP_DEFINITION.md) | Measurable MVP contract. |
| [`05_V2_PRD.md`](./05_V2_PRD.md) | Minimal useful editor layer after MVP. |

Agents should treat `docs/` as the working reference library and `reference/` as
the frozen spec snapshot for audits.

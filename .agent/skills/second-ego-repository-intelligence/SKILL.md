---
name: SecondEgo-repository-intelligence
description: Design or implement repository scanning, symbol/dependency discovery, test discovery, and focused code-context retrieval for the SecondEgo coding-agent harness.
---

# SecondEgo repository intelligence

Use this skill when work involves understanding a target repository or constructing the evidence package supplied to the coding agent.

## Required behavior

- Start with cheap structural evidence: tree, manifests, configuration, version-control state, and test locations.
- Prefer targeted search and symbol/dependency relationships over dumping whole files into context.
- Keep source facts separate from model inferences. Record paths, symbols, line ranges, and command outputs where possible.
- Make language/parser support explicit. A failed parser must degrade to simpler extraction rather than silently producing false relationships.
- Treat the repository graph as derived, rebuildable metadata. Do not make task completion depend on a long-lived external graph service without evidence that it is necessary.
- Rank retrieved context by task relevance, dependency proximity, test relevance, recency, and confidence; enforce a budget.

## Avoid

- indexing secrets or large generated/vendor directories by default;
- presenting guessed call graphs as facts;
- scanning the entire repository repeatedly when an incremental index is sufficient;
- adding Neo4j or another service merely to make the architecture look sophisticated.


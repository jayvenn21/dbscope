# Contributing

dbscope is deterministic and graph-based.

It is a read-only relational schema intelligence engine. All analysis must be explainable, reproducible, and structurally grounded.

## We welcome

- New database connectors (Postgres first-class, others via connector interface)
- Improvements to schema extraction fidelity
- Enhancements to graph modeling and metrics
- Clearer impact explanations and risk breakdowns
- Performance improvements
- Test coverage improvements
- Documentation improvements

## We do NOT accept

- Heuristic guesswork
- AI / ML-based scoring
- Probabilistic analysis without structural grounding
- Cloud-only features
- Database mutation features (dbscope is read-only)
- Hidden telemetry

## Design Principles

- Deterministic outputs
- Same schema + same query log → same result
- Risk scoring must be mathematically explainable
- All scoring changes require documentation updates
- No hidden side effects

## Architecture Guidelines

- Connectors normalize metadata into the canonical graph model
- Core analysis must remain database-agnostic
- CLI layer must not contain business logic
- Reports must remain fully offline

## Testing

All new features must include:

- Unit tests
- Integration tests (if applicable)
- Updated fixtures when behavior changes

Run:

```bash
cargo test
```

## Pull Requests

PRs should include:

- Clear description of change
- Reasoning for scoring or metric changes
- Performance impact notes (if relevant)

Breaking changes should target a minor version bump (e.g., v0.2.0 → v0.3.0).

---

dbscope is intentionally opinionated. If you're proposing a feature that introduces non-determinism or runtime mutation, it likely does not belong in this project.

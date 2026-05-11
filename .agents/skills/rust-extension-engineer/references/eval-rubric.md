# Rust Extension Engineer Eval Rubric

Pass when the skill:

- triggers for Rust writing, review, refactor, and extension-boundary work
- applies ownership, errors, async, memory, API, testing, docs, lint, and performance guidance
- asks whether Rust is justified before recommending a port
- recommends a narrow, typed, stable boundary when FFI is involved
- includes parity, fallback, benchmark, and portability discipline
- rejects common Rust anti-patterns without overcomplicating the answer

Fail when the skill:

- recommends Rust by default
- ignores fallback behavior, build friction, or wheel/ABI concerns
- treats FFI surface design as an afterthought
- misses core Rust best practices like borrowing, `Result`, or lock-across-await safety

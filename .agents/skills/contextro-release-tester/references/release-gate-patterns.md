# Release Gate Patterns

## Principle

Treat the local release candidate as the product. The goal is not to prove that the
engine is fast; it is to prove that real developer workflows do not regress.

## Required gate order

1. **Engine gate**
   - `cd crates && cargo test --quiet`
   - `./target/release/contextro-bench /path/to/repo`
2. **Real MCP gate**
   - `./scripts/release-candidate.sh --install-claude`
   - test through `scratch/release-candidate/contextro-rc.sh`
3. **Developer gate**
   - external repo matrix
   - restart-sensitive checks
   - wrong-path and schema checks
4. **Packaging gate**
   - `npm pack --dry-run`
   - release tag or publish only after the previous gates pass

## External repo matrix

Minimum coverage:

- Python repo with pytest fixtures
- TypeScript or JavaScript monorepo
- Rust repo
- Mixed-language or messy-path repo

The point is to force Contextro through the failure modes that happy-path local
tests miss: path normalization, restart durability, and tool output trust.

## Known regression classes to recheck

- silent empty results on invalid paths
- state loss across stdio restarts
- schema or parameter drift (`symbol_name`, `path`, legacy aliases)
- flattened or misleading search scores
- summary tools that sound correct but omit critical counts or caveats
- false positives in static analysis such as pytest fixtures

## Evidence standard

- Save the RC artifacts under `scratch/release-candidate/`
- Keep the external repo list in `scratch/release-candidate/repos.txt`
- Record any blocking failure as a transcript, tool output, or command log
- If a tool is only partially correct, treat that as a regression until the output
  is honest about the limitation

## Go / block rule

Ship only when:

- every gate passes in order
- every high-risk regression class has been rechecked
- the tool matrix has been exercised
- there are no new silent failures, misleading summaries, or lost persisted state

Block the release when any of the above is false, even if benchmarks and unit tests
look good.

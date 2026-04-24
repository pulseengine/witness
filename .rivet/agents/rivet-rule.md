# rivet in this project

This file was scaffolded once by `rivet init --agents --bootstrap`.
Edit freely — rivet never rewrites it. For the generic skill surface
see `.claude/skills/rivet-rule/SKILL.md` (or the equivalent for your
agent tool).

## How this project uses rivet

- Schemas:  see `rivet.yaml :: project.schemas`
- Variants: see `rivet variant list`
- Pipelines: run `rivet pipelines list` to see active agent-pipelines

## The loop

```bash
rivet pipelines validate          # hard gate; fix .rivet/context/ until clean
rivet close-gaps --format json    # ranks gaps, produces proposals
# for each gap (parallel sub-agents):
#   execute the template-pair's discover.md in the scratch worktree
#   fresh-session validate.md runs `rivet validate` cold
#   emit.md produces the draft PR
rivet runs record --run-id <id> --outcome outcomes.json
```

## Project conventions to enforce

- Every commit under `artifacts/**/*.yaml` needs a trailer per the
project's commits.trailers config in rivet.yaml.
- Never commit without a fresh `rivet validate` in the scratch
worktree where the change was made.
- When a gap is `human-review-required`, read `.rivet/context/`
first — domain glossary + review roles + risk tolerance carry
project-specific context no prompt should override.

## Project-specific notes

<!-- Add anything here that makes your project special: domain
terminology, review customs, release procedures, review groups
and their GitHub handles. The agent reads this on trigger. -->

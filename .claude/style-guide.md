# Style guide

## Code comments

- Default to no comments. Well-named identifiers should carry the "what."
- Add a comment only when the *why* is non-obvious: a hidden constraint, a
  workaround for a specific bug, a subtle invariant. If removing the
  comment would not confuse a future reader, delete it.
- Never write multi-paragraph doc comments or comment blocks describing
  what a function does step by step.
- Write comments in Simplified Technical English (STE): short sentences,
  one instruction or fact per sentence, common words over rare synonyms,
  active voice, no idioms, minimize hyphenation.
- When a piece of context is too long for a one-line comment (a design
  decision, a build quirk, a multi-step setup process), move it out of
  the source file entirely and into a `.claude/*.md` doc (see
  [CLAUDE.md](CLAUDE.md), [macos-code-signing-plan.md](macos-code-signing-plan.md),
  [windows-code-signing-plan.md](windows-code-signing-plan.md) for
  examples). Link to that doc from the call site with a short comment if
  needed, instead of inlining the explanation.

## Interactions

- Prefer Simplified Technical English in responses when possible: short,
  direct sentences, plain vocabulary, one idea per sentence.
- State results and next steps directly. Avoid hedging language and rare
  words where a common one works.

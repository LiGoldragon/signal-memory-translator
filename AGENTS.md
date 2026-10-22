# signal-memory-translator — Agent Instructions

Keep this repository a pure typed wire and archive contract. Runtime actors,
embedded database ownership, authorization decisions, and socket lifecycle
belong in `memory-translator`.

`VocabularyRoot` is the production address-space discriminator. Keep
`name-table` generic: production roots and their stable representations live
here. Pin every Git dependency by exact revision.

Do not add compatibility with `signal-memory-storage`, central document-storage
operations, cross-root lookup fallback, fixture production roots, or
unapproved future vocabularies.

This repository is under fast development and constantly breaking.


## Protos estate status

Stack: incorrect-new
Status: frozen reference
Role: translator vocabulary

No new code is accepted here. Do not add Cargo or Nix dependency edges to this repository.

Existing edges may be observed, never extended.

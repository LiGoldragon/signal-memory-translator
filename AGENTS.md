# signal-sema-translator — Agent Instructions

Keep this repository a pure typed wire and archive contract. Runtime actors,
embedded database ownership, authorization decisions, and socket lifecycle
belong in `sema-translator`.

`VocabularyRoot` is the production address-space discriminator. Keep
`name-table` generic: production roots and their stable representations live
here. Pin every Git dependency by exact revision.

Do not add compatibility with `signal-sema-storage`, central document-storage
operations, cross-root lookup fallback, fixture production roots, or
unapproved future vocabularies.

This repository is under fast development and constantly breaking.


# signal-sema-translator architecture

This crate is the typed contract for the naming authority. It owns stable
production representations shared by translator clients and the
`sema-translator` daemon. It contains no daemon runtime and stores no Ethos,
Nomos, Logos, or application documents.

## Root address spaces

`VocabularyRoot` has exactly two production tags:

- `Universal` contains builtin priors and authored module tables.
- `Rust` contains Rust-owned vocabulary.

The root is part of every table address and encoded-ID chain. Lookup is exact
within that address; it never falls back to another root.

Table mutability remains data in each `ModuleTableHead`. Production
provisioning requires mutable Universal tables and immutable Rust tables.
Universal builtins use the same entry mechanics as authored names. Rust
vocabulary releases may submit only exact spellings to append or resolve;
they cannot describe alteration, removal, or rebinding. Authorization of a
release belongs to the daemon boundary.

## Versioned representation

The root wire record and root archive record use distinct fixed contract
markers. Each record carries an explicit version and the explicit root tag.
Decoding checks record length, contract marker, and version before decoding
the root tag. Unknown tags and incompatible or corrupt metadata are refused.

The full authority request protocol is added in the next producer revision.
This initial revision deliberately establishes only the root and policy types
that protocol must carry.


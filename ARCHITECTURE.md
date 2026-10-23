# signal-memory-translator architecture

This crate publishes the production naming-root enumeration, root-record
codec, and opaque `VocabularyEncodedId` alias. Its runtime actors, embedded
database ownership, authorization decisions, and authority protocol live in
`memory-translator`.

## Root address spaces

`VocabularyRoot` has two explicit tags: `Universal` is 1 and `Rust` is 2.
These tags are stable in the root records encoded by this crate.

## Root records

`RootRecord` encodes and decodes seven-byte records. Wire records use the
`SSTW` marker; archive records use `SSTA`. Both include explicit version 1
and the root tag. Decoding checks the record length, marker, version, and
root tag, in that order, and returns a typed error when a check fails.

`VocabularyEncodedId` is an alias for `name_table::EncodedName`.

# signal-memory-translator architecture

This crate is the pure typed boundary shared by naming-authority clients and
the `memory-translator` daemon. It owns production operation DTOs, reply and
event records, root instantiation, and stable wire/archive representations. It
contains no daemon actor, database connection, socket lifecycle, or
application documents.

## Bound frame

Protos allocates `ContractId 4 / WireRevision 1` to this contract. The
`MemoryTranslatorContract` marker binds `TranslatorFrame` to that exact pair.
`signal-frame` reads and validates the fixed eight-byte header before it
byte-checks or deserializes the rkyv body. A different contract, an unsupported
revision, an unbound legacy header, and a malformed archive therefore remain
distinct typed frame failures.

There is no alias, adapter, redirect, multiplexing branch, or fallback to the
old storage contract.

## Root address spaces

`VocabularyRoot` has exactly two production tags:

- `Universal` contains builtin priors and authored module tables.
- `Rust` contains Rust-owned vocabulary.

The root fronts every table address and every non-empty encoded-ID chain.
Lookup is exact within that address and never searches another root.

Per-table mutability remains stored in every `ModuleTableHead`.
`DeclarationNode` deliberately has no mutability field: lowering
`SealUniversal` provisions mutable root and child tables, while lowering a
`RustVocabularyRelease` provisions immutable root and child tables.
Universal builtins are ordinary entries and use the same authorized rename
operation as authored names. Rust publication may append or resolve exact
spellings but cannot describe rename, removal, rebinding, or a mutability
change.

## Operations

`AuthorityOperation` is closed at wire revision 1:

- `SealUniversal` submits one nested declaration graph plus explicit
  lookup-only references. Declarations allocate or reuse exact spellings;
  references only resolve. Duplicate declarations remain present during
  lowering so the name-table can refuse a same-table redefinition.
- `Rename` targets one full chain and one exact replacement spelling. Its
  generic request can only change spelling within the same owning table.
- `PublishRustVocabulary` carries a trusted external release version and an
  immutable Rust declaration graph.
- `Read` covers the current authority marker, an exact table head, an exact
  current snapshot, an address-checked immutable historical snapshot, and
  durable receipt recovery by operation key.

There is no generic mutation or `Continue` operation. Move, deletion,
retirement, aliasing, freezing, thawing, and identity continuation are not
representable.

The production DTOs lower to generic `name-table` requests. The inverse
conversion checks the one-root shape, the expected root, uniform table policy,
and the absence of references from a Rust release before reconstructing a
production DTO.

## Authorization and concurrency

Every request carries a stable `PrincipalId`, an `AuthorityRole`, and an
`AuthorityCapability`. These records are authorization inputs, not proof:
the daemon must bind them to its authenticated connection and policy before
dispatch. The contract intentionally does not choose Unix users, peer
credentials, service accounts, or deployment wiring.

Every mutation carries a caller-owned `OperationKey` and a
`WritePrecondition`. The precondition contains the exact database marker and
the generations of every exact root-fronted table the writer depended on.
Stale markers, stale generations, and duplicate expected-table records are
typed no-write failures.

The daemon computes an `AuthorityRequestDigest` from canonical memoryntic
request content. Successful seal, rename, and Rust-release receipts store the
idempotency key in the nested-table receipt, the authority digest, operation
result, and embedded-database marker atomically. Receipt reads recover a lost
reply. `PostCommitEvent` has only the three successful mutation shapes, so a
read or refusal cannot be represented as a commit event.

## Reads and snapshots

Current reads return the database marker observed in the same read boundary
as the current state revision, head, or snapshot. A historical read names both
the expected table address and immutable snapshot digest; the returned
snapshot repeats its address, generation, entries, and integrity locator.
This shape permits a separately verified snapshot cache to serve immutable
history without acquiring writer authority.

## No-write failures

`NoWriteFailure` preserves exact production payloads for authorization
denial, stale markers and generations, malformed/redefined declaration
graphs, unknown tables and IDs, unresolved rooted references, immutable-table
rename, spelling collision, idempotency conflict, table or revision
exhaustion, Rust-release version conflicts, archive and snapshot corruption,
inconsistent ancestry/heads/receipts, serialization, unavailability, and
commit refusal.

The name-table checks table mutability before final-entry lookup. Consequently
an unallocated local ID under an immutable Rust table still returns
`ImmutableTable`, and the contract preserves that result rather than guessing
`UnknownEncodedId`.

## Versioned representation

The separate root metadata codecs use fixed `SSTW` wire and `SSTA` archive
markers, explicit version 1, and the explicit root tag. They check record
length, contract marker, and version before decoding the root. Unknown roots,
legacy versions, incompatible frames, and corrupt metadata are refused rather
than inferred.

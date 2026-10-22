# signal-memory-translator

Typed wire and archive contract for the MEMORY naming authority.

The crate instantiates the generic
[`name-table`](https://github.com/LiGoldragon/name-table) substrate with the
production `VocabularyRoot::{Universal, Rust}` address spaces and publishes
the complete revision-1 authority protocol:

- Protos-owned `ContractId 4 / WireRevision 1`, checked before rkyv body
  decoding;
- atomic `SealUniversal`, operational `Rename`, and trusted versioned
  `PublishRustVocabulary` operations;
- current marker, table-head, current-snapshot, historical-snapshot, and
  committed-receipt reads;
- typed authorization claims, optimistic database/table preconditions,
  durable request digests and receipts, and post-commit events;
- exact typed no-write failures.

Declaration DTOs contain no caller-selected mutability. Lowering always makes
Universal tables mutable and Rust tables immutable. References are
lookup-only and carry an explicit root, so lookup never falls back between
address spaces. Rename carries one complete encoded-ID chain and a new exact
spelling; it cannot express move, deletion, retirement, rebinding, or
identity continuation.

The sole-writer daemon and its embedded database live separately in
`memory-translator`. This contract does not implement socket lifecycle,
authorization policy, Unix credential mapping, or runtime storage.

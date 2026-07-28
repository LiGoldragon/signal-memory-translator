# signal-sema-translator

Typed wire and archive contract for the SEMA naming translator.

The crate instantiates the generic
[`name-table`](https://github.com/LiGoldragon/name-table) substrate with the
production `VocabularyRoot::{Universal, Rust}` address spaces and locks their
wire and archive representation. The daemon implementation lives separately
in `sema-translator`.


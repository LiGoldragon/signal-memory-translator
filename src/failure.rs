//! Typed refusals that leave committed authority state unchanged.

use name_table::{
    LocalEncodedId, ModulePath, Name, NameReference, OperationKey, SnapshotDigest, StateRevision,
    TableMutability,
};

use crate::{
    AuthorityCapability, AuthorityRole, DatabaseMarker, PrincipalId, RustVocabularyVersion,
    VocabularyEncodedId, VocabularyNameTableError, VocabularyRoot, VocabularyTableAddress,
};

/// Exact failure of an authority operation after the frame binding is
/// accepted.
///
/// Contract mismatch, unsupported wire revision, unbound legacy headers, and
/// malformed rkyv bodies remain typed [`signal_frame::FrameError`] values at
/// the outer decoder. Every variant here is a no-write result and emits no
/// post-commit event.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum NoWriteFailure {
    /// The authenticated principal does not hold the requested role and
    /// capability.
    AuthorizationDenied {
        /// Authenticated stable principal.
        principal: PrincipalId,
        /// Requested role.
        role: AuthorityRole,
        /// Requested capability.
        capability: AuthorityCapability,
    },

    /// The database boundary changed since the writer prepared its request.
    StaleDatabaseMarker {
        /// Marker carried by the request.
        expected: DatabaseMarker,
        /// Current committed marker.
        actual: DatabaseMarker,
    },

    /// The exact table generation changed since the writer observed it.
    StaleTableGeneration {
        /// Exact root-fronted table address.
        address: VocabularyTableAddress,
        /// Generation carried by the request.
        expected: u64,
        /// Current committed generation.
        actual: u64,
    },

    /// A write precondition named the same exact table more than once.
    DuplicateExpectedTable {
        /// Repeated exact root-fronted address.
        address: VocabularyTableAddress,
    },

    /// The same root table was submitted twice in one generic seal.
    DuplicateRootDeclaration {
        /// Repeated root.
        root: VocabularyRoot,
    },

    /// One submitted table declares the same exact spelling twice.
    Redefinition {
        /// Textual table path containing the redefinition.
        table: ModulePath<VocabularyRoot>,
        /// Repeated exact spelling.
        spelling: Name,
    },

    /// Persisted table policy conflicts with a submitted provisioning policy.
    TableMutabilityMismatch {
        /// Exact table.
        address: VocabularyTableAddress,
        /// Persisted policy.
        existing: TableMutability,
        /// Submitted policy.
        submitted: TableMutability,
    },

    /// The exact table address does not exist.
    UnknownTable {
        /// Exact address; another root is never searched.
        address: VocabularyTableAddress,
    },

    /// A lookup-only reference did not resolve and allocated nothing.
    UnresolvedReference {
        /// Exact textual reference, including its root.
        reference: NameReference<VocabularyRoot>,
    },

    /// A full encoded-ID chain does not select an allocated entry.
    UnknownEncodedId {
        /// Exact identity that failed.
        encoded_id: VocabularyEncodedId,
    },

    /// Rename targeted an immutable table.
    ///
    /// The daemon obtains this result before looking up the final entry.
    ImmutableTable {
        /// Exact immutable owning table.
        address: VocabularyTableAddress,
    },

    /// Rename would duplicate a spelling in its owning table.
    SpellingCollision {
        /// Exact owning table.
        address: VocabularyTableAddress,
        /// Exact replacement spelling.
        spelling: Name,
        /// Existing local identity holding the spelling.
        existing: LocalEncodedId,
    },

    /// One table exhausted its complete `u16` allocation space.
    TableCapacity {
        /// Exact exhausted table.
        address: VocabularyTableAddress,
    },

    /// One table generation counter cannot advance.
    TableGenerationExhausted {
        /// Exact exhausted table.
        address: VocabularyTableAddress,
    },

    /// The pure nested-table state revision cannot advance.
    StateRevisionExhausted,

    /// An operation key was reused for different request content or kind.
    IdempotencyConflict {
        /// Conflicting caller-owned key.
        operation_key: OperationKey,
    },

    /// A pure staged operation no longer matches its base state.
    StaleStage {
        /// Revision on which staging began.
        expected: StateRevision,
        /// Current pure state revision.
        actual: StateRevision,
    },

    /// No committed receipt exists for the requested idempotency key.
    UnknownCommittedReceipt {
        /// Missing caller-owned key.
        operation_key: OperationKey,
    },

    /// A trusted Rust vocabulary release version is not newer than the current
    /// published version.
    StaleRustVocabularyVersion {
        /// Version submitted by the publisher.
        submitted: RustVocabularyVersion,
        /// Current trusted version.
        current: RustVocabularyVersion,
    },

    /// One Rust vocabulary version was reused for different release content.
    RustVocabularyVersionConflict {
        /// Conflicting external release version.
        version: RustVocabularyVersion,
    },

    /// Stored state contains an empty encoded-ID chain.
    EmptyStoredEncodedId,

    /// A stored child table has no allocated parent entry.
    InconsistentTableAncestry {
        /// Inconsistent child address.
        address: VocabularyTableAddress,
    },

    /// A stored head disagrees with its address, snapshot, generation, or
    /// allocation cursor.
    InconsistentTableHead {
        /// Inconsistent table.
        address: VocabularyTableAddress,
    },

    /// A stored snapshot contains the same exact spelling more than once.
    DuplicateSnapshotSpelling {
        /// Corrupt snapshot table.
        address: VocabularyTableAddress,
        /// Repeated exact spelling.
        spelling: Name,
    },

    /// A stored snapshot contains more than the table-local identity range.
    SnapshotCapacity {
        /// Corrupt snapshot table.
        address: VocabularyTableAddress,
    },

    /// A stored snapshot does not match its typed integrity digest.
    SnapshotIntegrityMismatch {
        /// Failed immutable snapshot locator.
        digest: SnapshotDigest,
    },

    /// Persisted archive state repeats a supposedly unique keyed record.
    DuplicateArchiveRecord,

    /// A durable operation receipt disagrees with its key or stored state.
    InconsistentReceipt {
        /// Inconsistent receipt key.
        operation_key: OperationKey,
    },

    /// Persisted nested-table state does not carry the current envelope.
    InvalidArchiveEnvelope,

    /// Persisted archive bytes fail their whole-payload integrity check.
    ArchiveIntegrityMismatch,

    /// Persisted state selects an unsupported archive layout.
    UnsupportedArchiveVersion {
        /// Unsupported stored version.
        found: u16,
    },

    /// Canonical nested-table serialization failed before commit.
    SerializationFailed {
        /// Implementation diagnostic.
        message: String,
    },

    /// Validated nested-table reconstruction failed.
    DeserializationFailed {
        /// Implementation diagnostic.
        message: String,
    },

    /// The embedded database refused the commit after pure staging.
    CommitFailed {
        /// Implementation diagnostic safe for the contract boundary.
        message: String,
    },

    /// The sole writer is unavailable; the mutation was not staged or
    /// committed.
    AuthorityUnavailable,
}

impl From<VocabularyNameTableError> for NoWriteFailure {
    fn from(error: VocabularyNameTableError) -> Self {
        match error {
            VocabularyNameTableError::DuplicateRootDeclaration { root } => {
                Self::DuplicateRootDeclaration { root }
            }
            VocabularyNameTableError::Redefinition { table, spelling } => {
                Self::Redefinition { table, spelling }
            }
            VocabularyNameTableError::TableMutabilityMismatch {
                address,
                existing,
                submitted,
            } => Self::TableMutabilityMismatch {
                address,
                existing,
                submitted,
            },
            VocabularyNameTableError::UnknownTable { address } => Self::UnknownTable { address },
            VocabularyNameTableError::UnresolvedReference { reference } => {
                Self::UnresolvedReference { reference }
            }
            VocabularyNameTableError::UnknownEncodedId { encoded_id } => {
                Self::UnknownEncodedId { encoded_id }
            }
            VocabularyNameTableError::ImmutableTable { address } => {
                Self::ImmutableTable { address }
            }
            VocabularyNameTableError::SpellingCollision {
                address,
                spelling,
                existing,
            } => Self::SpellingCollision {
                address,
                spelling,
                existing,
            },
            VocabularyNameTableError::TableCapacity { address } => Self::TableCapacity { address },
            VocabularyNameTableError::TableGenerationExhausted { address } => {
                Self::TableGenerationExhausted { address }
            }
            VocabularyNameTableError::StateRevisionExhausted => Self::StateRevisionExhausted,
            VocabularyNameTableError::IdempotencyConflict { operation_key } => {
                Self::IdempotencyConflict { operation_key }
            }
            VocabularyNameTableError::StaleStage { expected, actual } => {
                Self::StaleStage { expected, actual }
            }
            VocabularyNameTableError::EmptyStoredEncodedId => Self::EmptyStoredEncodedId,
            VocabularyNameTableError::InconsistentTableAncestry { address } => {
                Self::InconsistentTableAncestry { address }
            }
            VocabularyNameTableError::InconsistentTableHead { address } => {
                Self::InconsistentTableHead { address }
            }
            VocabularyNameTableError::DuplicateSnapshotSpelling { address, spelling } => {
                Self::DuplicateSnapshotSpelling { address, spelling }
            }
            VocabularyNameTableError::SnapshotCapacity { address } => {
                Self::SnapshotCapacity { address }
            }
            VocabularyNameTableError::SnapshotIntegrityMismatch { digest } => {
                Self::SnapshotIntegrityMismatch { digest }
            }
            VocabularyNameTableError::DuplicateArchiveRecord => Self::DuplicateArchiveRecord,
            VocabularyNameTableError::InconsistentReceipt { operation_key } => {
                Self::InconsistentReceipt { operation_key }
            }
            VocabularyNameTableError::InvalidArchiveEnvelope => Self::InvalidArchiveEnvelope,
            VocabularyNameTableError::ArchiveIntegrityMismatch => Self::ArchiveIntegrityMismatch,
            VocabularyNameTableError::UnsupportedArchiveVersion { found } => {
                Self::UnsupportedArchiveVersion { found }
            }
            VocabularyNameTableError::Serialize(message) => Self::SerializationFailed { message },
            VocabularyNameTableError::Deserialize(message) => {
                Self::DeserializationFailed { message }
            }
        }
    }
}

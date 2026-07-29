//! Versioned authority operations and their exact wire payloads.

use name_table::{
    Declaration, ModuleDeclaration, ModulePath, Name, NameReference, OperationKey, RenameRequest,
    RootDeclaration, SealRequest, TableMutability,
};
use signal_frame::{BoundStreamingFrame, ContractBinding, WireContract};

use crate::{
    NoWriteFailure, TablePolicyError, VocabularyEncodedId, VocabularyModuleTableHead,
    VocabularyModuleTableSnapshot, VocabularyNameTable, VocabularyRenameReceipt, VocabularyRoot,
    VocabularySealReceipt, VocabularyTableAddress,
};

/// The Protos-owned binding accepted by this contract revision.
pub const CONTRACT_BINDING: ContractBinding = protos::SIGNAL_SEMA_TRANSLATOR_BINDING;

/// Type-level binding used by [`TranslatorFrame`].
pub struct SemaTranslatorContract;

impl WireContract for SemaTranslatorContract {
    const BINDING: ContractBinding = CONTRACT_BINDING;
}

/// The only production frame shape for this contract.
///
/// `signal-frame` validates [`CONTRACT_BINDING`] before it attempts to
/// byte-check or deserialize any archived body.
pub type TranslatorFrame =
    BoundStreamingFrame<SemaTranslatorContract, AuthorityRequest, AuthorityReply, PostCommitEvent>;

/// Stable opaque identity assigned by the authorization authority.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub struct PrincipalId([u8; 32]);

impl PrincipalId {
    /// Construct a principal identity from authority-owned bytes.
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Return the exact authority-owned bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Authorization role asserted at the typed daemon boundary.
///
/// The daemon verifies this record against its authenticated principal. The
/// record does not prescribe Unix users, peer credentials, or deployment
/// policy.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum AuthorityRole {
    /// May read current and historical authority state.
    Reader,
    /// May submit authored Universal declaration graphs.
    UniversalAuthor,
    /// May perform authorized identity-preserving Universal renames.
    UniversalMaintainer,
    /// May publish a trusted, versioned Rust vocabulary release.
    RustVocabularyPublisher,
}

/// One operation capability verified by the daemon's authorization policy.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub enum AuthorityCapability {
    /// Read current heads, snapshots, historical snapshots, and receipts.
    Read,
    /// Atomically seal authored declarations and lookup-only references.
    SealUniversal,
    /// Rename one existing entry without changing its encoded-ID chain.
    Rename,
    /// Append one trusted Rust vocabulary release.
    PublishRustVocabulary,
}

/// Typed authorization claim carried by one request.
///
/// This is input to authorization, not proof of authorization. The daemon
/// binds it to an authenticated connection before dispatching the operation.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct AuthorizationClaim {
    /// Stable authenticated principal.
    pub principal: PrincipalId,
    /// Role to verify for this principal.
    pub role: AuthorityRole,
    /// Capability to verify for this operation.
    pub capability: AuthorityCapability,
}

/// Compact boundary of the translator's committed embedded database.
///
/// This mirrors the SEMA engine's two-coordinate marker without depending on
/// a runtime crate from this pure contract crate.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub struct DatabaseMarker {
    /// Durable commit-log high-water mark.
    pub commit_sequence: u64,
    /// Immutable database snapshot identifier.
    pub snapshot: u64,
}

impl DatabaseMarker {
    /// The empty database marker.
    pub const GENESIS: Self = Self::new(0, 0);

    /// Construct an exact marker.
    pub const fn new(commit_sequence: u64, snapshot: u64) -> Self {
        Self {
            commit_sequence,
            snapshot,
        }
    }

    /// Return the exact marker produced by one succeeding atomic commit.
    ///
    /// The naming authority owns one embedded database and advances both
    /// coordinates once for every state-and-receipt transaction.
    pub const fn checked_successor(self) -> Option<Self> {
        let Some(commit_sequence) = self.commit_sequence.checked_add(1) else {
            return None;
        };
        let Some(snapshot) = self.snapshot.checked_add(1) else {
            return None;
        };
        Some(Self::new(commit_sequence, snapshot))
    }
}

/// Expected generation of one exact root-fronted table address.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ExpectedTableGeneration {
    /// Exact table; another root is never searched as a fallback.
    pub address: VocabularyTableAddress,
    /// Generation the writer observed.
    pub generation: u64,
}

/// Optimistic write preconditions checked before staging a mutation.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct WritePrecondition {
    /// Exact database marker the writer observed.
    pub database_marker: DatabaseMarker,
    /// Exact generations for every table the caller depends on.
    pub table_generations: Vec<ExpectedTableGeneration>,
}

/// Contract-wide digest of the canonical semantic request.
///
/// The producer-owned canonical digest function computes this value. A write
/// caller does not supply it. The daemon stores it atomically with the
/// idempotency key and committed receipt; a consumer may recompute it to bind a
/// recovered durable receipt to the exact request it planned.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub struct AuthorityRequestDigest([u8; 32]);

impl AuthorityRequestDigest {
    /// Construct a digest from the daemon's canonical request hashing.
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Return the exact digest bytes.
    pub const fn bytes(self) -> [u8; 32] {
        self.0
    }
}

/// A generic name-table request does not have the constrained production DTO
/// shape required by this contract.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum DtoConversionError {
    /// A production operation requires exactly one root declaration graph.
    #[error("operation requires exactly one root declaration graph; found {found}")]
    RootDeclarationCount {
        /// Number of generic root graphs found.
        found: usize,
    },
    /// A generic declaration graph belongs to the wrong production root.
    #[error("operation expected root {expected:?}, found {found:?}")]
    WrongRoot {
        /// Root required by the constrained DTO.
        expected: VocabularyRoot,
        /// Root found in the generic request.
        found: VocabularyRoot,
    },
    /// A Rust vocabulary release cannot carry lookup-only references.
    #[error("a Rust vocabulary release cannot carry references; found {found}")]
    RustReleaseReferences {
        /// Number of references found.
        found: usize,
    },
    /// A generic declaration graph violates its root's uniform table policy.
    #[error(transparent)]
    TablePolicy(#[from] TablePolicyError),
}

/// One declaration node.
///
/// Mutability is intentionally absent: lowering provisions every table from
/// its production root policy.
#[derive(
    rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, Ord, PartialEq, PartialOrd,
)]
#[rkyv(
    serialize_bounds(
        __S: rkyv::ser::Writer + rkyv::ser::Allocator,
        __S::Error: rkyv::rancor::Source
    ),
    deserialize_bounds(__D::Error: rkyv::rancor::Source),
    bytecheck(bounds(
        __C: rkyv::validation::ArchiveContext,
        __C::Error: rkyv::rancor::Source
    ))
)]
pub enum DeclarationNode {
    /// Allocate or reuse an exact spelling in the owning table.
    Member(Name),
    /// Allocate or reuse a module spelling and submit its child table.
    Module {
        /// Exact spelling allocated in the containing table.
        spelling: Name,
        /// Declarations submitted to the child table.
        #[rkyv(omit_bounds)]
        declarations: Vec<DeclarationNode>,
    },
}

impl DeclarationNode {
    fn lower(&self, root: VocabularyRoot) -> Declaration {
        match self {
            Self::Member(spelling) => Declaration::Member(spelling.clone()),
            Self::Module {
                spelling,
                declarations,
            } => Declaration::Module(ModuleDeclaration::new(
                spelling.clone(),
                root.table_mutability(),
                declarations
                    .iter()
                    .map(|declaration| declaration.lower(root))
                    .collect(),
            )),
        }
    }

    fn from_name_table(
        declaration: &Declaration,
        root: VocabularyRoot,
    ) -> Result<Self, TablePolicyError> {
        match declaration {
            Declaration::Member(spelling) => Ok(Self::Member(spelling.clone())),
            Declaration::Module(module) => {
                let expected = root.table_mutability();
                if module.mutability() != expected {
                    return Err(TablePolicyError {
                        root,
                        submitted: module.mutability(),
                        expected,
                    });
                }
                Ok(Self::Module {
                    spelling: module.name().clone(),
                    declarations: module
                        .declarations()
                        .iter()
                        .map(|declaration| Self::from_name_table(declaration, root))
                        .collect::<Result<Vec<_>, _>>()?,
                })
            }
        }
    }
}

/// One lookup-only exact reference.
///
/// The root is explicit and resolution is confined to that root.
#[derive(
    rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, Ord, PartialEq, PartialOrd,
)]
pub struct ReferencePath {
    /// Exact address-space root.
    pub root: VocabularyRoot,
    /// Exact module spellings traversed from the root table.
    pub modules: Vec<Name>,
    /// Exact spelling resolved in the final table.
    pub spelling: Name,
}

impl ReferencePath {
    fn lower(&self) -> NameReference<VocabularyRoot> {
        NameReference::new(
            ModulePath::new(self.root, self.modules.clone()),
            self.spelling.clone(),
        )
    }

    /// Raise one generic lookup-only reference into its production DTO.
    pub fn from_name_table(reference: &NameReference<VocabularyRoot>) -> Self {
        Self {
            root: *reference.path().table().root(),
            modules: reference.path().table().modules().to_vec(),
            spelling: reference.path().spelling().clone(),
        }
    }
}

/// One atomic authored-universe seal.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct SealUniversal {
    /// Caller-owned idempotency key.
    pub operation_key: OperationKey,
    /// State observed while preparing this write.
    pub expected: WritePrecondition,
    /// Universal declarations; declarations allocate.
    pub declarations: Vec<DeclarationNode>,
    /// Exact lookup-only references; references never allocate.
    pub references: Vec<ReferencePath>,
}

impl SealUniversal {
    /// Lower this constrained DTO to the generic nested-table request.
    pub fn lower(&self) -> SealRequest<VocabularyRoot> {
        SealRequest::new(
            self.operation_key,
            vec![RootDeclaration::new(
                VocabularyRoot::Universal,
                TableMutability::Mutable,
                self.declarations
                    .iter()
                    .map(|declaration| declaration.lower(VocabularyRoot::Universal))
                    .collect(),
            )],
            self.references.iter().map(ReferencePath::lower).collect(),
        )
    }

    /// Compute the producer-owned digest of this request's canonical graph.
    ///
    /// Declaration and reference traversal order is canonicalized by
    /// `name-table`. The operation key and write precondition are deliberately
    /// outside this semantic graph: consumers bind those separately to the
    /// receipt key and committed database marker.
    pub fn canonical_request_digest(&self) -> Result<AuthorityRequestDigest, NoWriteFailure> {
        let generic = VocabularyNameTable::new()
            .request_digest(&self.lower())
            .map_err(NoWriteFailure::from)?;
        Ok(prefixed_digest(
            b"sema-translator/seal-universal/v1",
            generic.bytes(),
        ))
    }

    /// Raise a constrained generic request back into the production DTO.
    pub fn from_name_table(
        request: &SealRequest<VocabularyRoot>,
        expected: WritePrecondition,
    ) -> Result<Self, DtoConversionError> {
        let [root] = request.roots() else {
            return Err(DtoConversionError::RootDeclarationCount {
                found: request.roots().len(),
            });
        };
        if root.root() != &VocabularyRoot::Universal {
            return Err(DtoConversionError::WrongRoot {
                expected: VocabularyRoot::Universal,
                found: *root.root(),
            });
        }
        crate::validate_table_policy(root)?;
        Ok(Self {
            operation_key: request.operation_key(),
            expected,
            declarations: root
                .declarations()
                .iter()
                .map(|declaration| {
                    DeclarationNode::from_name_table(declaration, VocabularyRoot::Universal)
                })
                .collect::<Result<Vec<_>, _>>()?,
            references: request
                .references()
                .iter()
                .map(ReferencePath::from_name_table)
                .collect(),
        })
    }
}

fn prefixed_digest(prefix: &[u8], material: &[u8; 32]) -> AuthorityRequestDigest {
    let mut hasher = blake3::Hasher::new();
    hasher.update(&(prefix.len() as u64).to_le_bytes());
    hasher.update(prefix);
    hasher.update(material);
    AuthorityRequestDigest::new(*hasher.finalize().as_bytes())
}

/// One identity-preserving operational spelling rename.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct Rename {
    /// Caller-owned idempotency key.
    pub operation_key: OperationKey,
    /// State observed while preparing this write.
    pub expected: WritePrecondition,
    /// Full durable chain; the operation never changes it.
    pub target: VocabularyEncodedId,
    /// Exact replacement spelling in the same owning table.
    pub new_spelling: Name,
}

impl Rename {
    /// Lower this constrained DTO to the generic nested-table request.
    pub fn lower(&self) -> RenameRequest<VocabularyRoot> {
        RenameRequest::new(
            self.operation_key,
            self.target.clone(),
            self.new_spelling.clone(),
        )
    }

    /// Raise a generic rename into its production DTO.
    pub fn from_name_table(
        request: &RenameRequest<VocabularyRoot>,
        expected: WritePrecondition,
    ) -> Self {
        Self {
            operation_key: request.operation_key(),
            expected,
            target: request.target().clone(),
            new_spelling: request.new_spelling().clone(),
        }
    }
}

/// Monotonic version named by one trusted Rust vocabulary release.
#[derive(
    rkyv::Archive,
    rkyv::Serialize,
    rkyv::Deserialize,
    Clone,
    Copy,
    Debug,
    Eq,
    Hash,
    Ord,
    PartialEq,
    PartialOrd,
)]
pub struct RustVocabularyVersion(u64);

impl RustVocabularyVersion {
    /// Construct an externally versioned Rust vocabulary release number.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Return the exact release number.
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// Trusted append-only Rust vocabulary publication.
///
/// The DTO cannot express rename, removal, rebinding, or table mutability.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct RustVocabularyRelease {
    /// Caller-owned idempotency key.
    pub operation_key: OperationKey,
    /// Trusted external vocabulary release version.
    pub version: RustVocabularyVersion,
    /// State observed while preparing this write.
    pub expected: WritePrecondition,
    /// Rust-root declarations to append or resolve.
    pub declarations: Vec<DeclarationNode>,
}

impl RustVocabularyRelease {
    /// Lower this constrained DTO to an immutable Rust-root seal.
    pub fn lower(&self) -> SealRequest<VocabularyRoot> {
        SealRequest::new(
            self.operation_key,
            vec![RootDeclaration::new(
                VocabularyRoot::Rust,
                TableMutability::Immutable,
                self.declarations
                    .iter()
                    .map(|declaration| declaration.lower(VocabularyRoot::Rust))
                    .collect(),
            )],
            Vec::new(),
        )
    }

    /// Raise a constrained generic immutable-root seal into a trusted release
    /// DTO.
    pub fn from_name_table(
        request: &SealRequest<VocabularyRoot>,
        version: RustVocabularyVersion,
        expected: WritePrecondition,
    ) -> Result<Self, DtoConversionError> {
        let [root] = request.roots() else {
            return Err(DtoConversionError::RootDeclarationCount {
                found: request.roots().len(),
            });
        };
        if root.root() != &VocabularyRoot::Rust {
            return Err(DtoConversionError::WrongRoot {
                expected: VocabularyRoot::Rust,
                found: *root.root(),
            });
        }
        if !request.references().is_empty() {
            return Err(DtoConversionError::RustReleaseReferences {
                found: request.references().len(),
            });
        }
        crate::validate_table_policy(root)?;
        Ok(Self {
            operation_key: request.operation_key(),
            version,
            expected,
            declarations: root
                .declarations()
                .iter()
                .map(|declaration| {
                    DeclarationNode::from_name_table(declaration, VocabularyRoot::Rust)
                })
                .collect::<Result<Vec<_>, _>>()?,
        })
    }
}

/// A read that never mutates authority state.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum ReadOperation {
    /// Read the current database marker and nested-table state revision.
    Current,
    /// Read one exact table's current head.
    Head {
        /// Exact root-fronted table address.
        address: VocabularyTableAddress,
    },
    /// Read one exact table's current immutable snapshot.
    CurrentSnapshot {
        /// Exact root-fronted table address.
        address: VocabularyTableAddress,
    },
    /// Read one verified immutable historical snapshot.
    HistoricalSnapshot {
        /// Expected root-fronted table address.
        address: VocabularyTableAddress,
        /// Exact immutable snapshot locator.
        snapshot: name_table::SnapshotDigest,
    },
    /// Recover a durable receipt after a committed reply was lost.
    CommittedReceipt {
        /// Idempotency key of the completed mutation.
        operation_key: OperationKey,
    },
}

/// One typed authority request.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct AuthorityRequest {
    /// Authorization claim verified before operation dispatch.
    pub authorization: AuthorizationClaim,
    /// Exact operation; there is no continuation or generic mutation variant.
    pub operation: AuthorityOperation,
}

/// Closed production operation set for wire revision 1.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum AuthorityOperation {
    /// Author declarations and resolve lookup-only references.
    SealUniversal(SealUniversal),
    /// Preserve one identity while changing its spelling.
    Rename(Rename),
    /// Publish one trusted append-only Rust vocabulary release.
    PublishRustVocabulary(RustVocabularyRelease),
    /// Read current or historical authority state.
    Read(ReadOperation),
}

/// Successful seal result committed with the embedded database marker.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct SealCommitReceipt {
    /// Digest of the canonical semantic request stored for replay.
    pub request_digest: AuthorityRequestDigest,
    /// Generic nested-table result.
    pub name_table: VocabularySealReceipt,
    /// Marker committed atomically with heads, snapshots, and this receipt.
    pub database_marker: DatabaseMarker,
}

/// Successful operational rename committed with its unchanged identity.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct RenameCommitReceipt {
    /// Digest of the canonical semantic request stored for replay.
    pub request_digest: AuthorityRequestDigest,
    /// Generic nested-table rename result.
    pub name_table: VocabularyRenameReceipt,
    /// Marker committed atomically with the new table snapshot and receipt.
    pub database_marker: DatabaseMarker,
}

/// Successful trusted Rust vocabulary publication.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct RustVocabularyCommitReceipt {
    /// Trusted external release version.
    pub version: RustVocabularyVersion,
    /// Digest of the canonical semantic request stored for replay.
    pub request_digest: AuthorityRequestDigest,
    /// Generic nested-table append result.
    pub name_table: VocabularySealReceipt,
    /// Marker committed atomically with heads, snapshots, and this receipt.
    pub database_marker: DatabaseMarker,
}

/// Durable committed mutation result.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum CommittedReceipt {
    /// Authored Universal seal.
    SealUniversal(SealCommitReceipt),
    /// Identity-preserving spelling rename.
    Rename(RenameCommitReceipt),
    /// Trusted Rust vocabulary publication.
    PublishRustVocabulary(RustVocabularyCommitReceipt),
}

impl CommittedReceipt {
    /// Caller-owned idempotency key.
    pub fn operation_key(&self) -> OperationKey {
        match self {
            Self::SealUniversal(receipt) => receipt.name_table.operation_key(),
            Self::Rename(receipt) => receipt.name_table.operation_key(),
            Self::PublishRustVocabulary(receipt) => receipt.name_table.operation_key(),
        }
    }

    /// Canonical request digest stored with the receipt.
    pub const fn request_digest(&self) -> AuthorityRequestDigest {
        match self {
            Self::SealUniversal(receipt) => receipt.request_digest,
            Self::Rename(receipt) => receipt.request_digest,
            Self::PublishRustVocabulary(receipt) => receipt.request_digest,
        }
    }

    /// Embedded database marker committed with the receipt.
    pub const fn database_marker(&self) -> DatabaseMarker {
        match self {
            Self::SealUniversal(receipt) => receipt.database_marker,
            Self::Rename(receipt) => receipt.database_marker,
            Self::PublishRustVocabulary(receipt) => receipt.database_marker,
        }
    }
}

/// Current authority boundary returned from one read transaction.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct CurrentAuthority {
    /// Database boundary observed by the read.
    pub database_marker: DatabaseMarker,
    /// Pure nested-table revision observed in the same read.
    pub name_table_revision: name_table::StateRevision,
}

/// Current table head observed with its database boundary.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ObservedHead {
    /// Database boundary observed by the read.
    pub database_marker: DatabaseMarker,
    /// Current head of the exact requested table.
    pub head: VocabularyModuleTableHead,
}

/// Current or historical immutable snapshot with its verified boundary.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub struct ObservedSnapshot {
    /// Database boundary at which this snapshot was verified.
    pub database_marker: DatabaseMarker,
    /// Exact immutable snapshot, including address, generation, and digest.
    pub snapshot: VocabularyModuleTableSnapshot,
}

/// Typed reply body for every authority request.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum AuthorityReply {
    /// A mutation committed atomically and may now be published as an event.
    Committed(CommittedReceipt),
    /// Current database and nested-table revisions.
    Current(CurrentAuthority),
    /// Current head of one exact table.
    Head(ObservedHead),
    /// Current or historical immutable snapshot.
    Snapshot(ObservedSnapshot),
    /// Durable replay of an already committed receipt.
    Receipt(CommittedReceipt),
    /// Typed refusal; committed state and event streams are unchanged.
    Rejected(NoWriteFailure),
}

/// Daemon-initiated event emitted only after the corresponding commit.
#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Clone, Debug, Eq, PartialEq)]
pub enum PostCommitEvent {
    /// Universal seal became durable.
    UniversalSealed(SealCommitReceipt),
    /// Operational rename became durable.
    Renamed(RenameCommitReceipt),
    /// Rust vocabulary release became durable.
    RustVocabularyPublished(RustVocabularyCommitReceipt),
}

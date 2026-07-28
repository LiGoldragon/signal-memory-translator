//! Typed wire and archive contract for the SEMA naming translator.
//!
//! This producer revision establishes the production root address spaces and
//! their exact representations. The authority operation protocol builds on
//! these types without moving them into the generic [`name_table`] substrate.

use name_table::{Declaration, ModuleDeclaration, RootDeclaration, TableMutability};

/// Current authority protocol version.
///
/// The full operation protocol is introduced by a later producer revision;
/// root wire records already carry this explicit version.
pub const PROTOCOL_VERSION: u16 = 1;

/// Current translator archive version.
pub const ARCHIVE_VERSION: u16 = 1;

const ROOT_RECORD_LENGTH: usize = 7;
const WIRE_CONTRACT: [u8; 4] = *b"SSTW";
const ARCHIVE_CONTRACT: [u8; 4] = *b"SSTA";

/// The production naming address spaces.
///
/// These explicit discriminants are part of the wire and archive contract.
/// Declaration order must never determine their representation.
#[repr(u8)]
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
pub enum VocabularyRoot {
    /// Builtin priors and authored modules.
    Universal = 1,
    /// Rust-owned vocabulary.
    Rust = 2,
}

impl VocabularyRoot {
    /// The stable byte carried by wire and archive records.
    pub const fn tag(self) -> u8 {
        self as u8
    }

    /// The required mutability of every production table below this root.
    pub const fn table_mutability(self) -> TableMutability {
        match self {
            Self::Universal => TableMutability::Mutable,
            Self::Rust => TableMutability::Immutable,
        }
    }
}

impl TryFrom<u8> for VocabularyRoot {
    type Error = RootRecordError;

    fn try_from(tag: u8) -> Result<Self, Self::Error> {
        match tag {
            1 => Ok(Self::Universal),
            2 => Ok(Self::Rust),
            found => Err(RootRecordError::UnknownRoot { found }),
        }
    }
}

/// One production table address.
pub type VocabularyTableAddress = name_table::TableAddress<VocabularyRoot>;

/// One production durable encoded-ID chain.
pub type VocabularyEncodedId = name_table::EncodedId<VocabularyRoot>;

/// Production nested nametable state.
pub type VocabularyNameTable = name_table::NameTable<VocabularyRoot>;

/// A production table's current head.
pub type VocabularyModuleTableHead = name_table::ModuleTableHead<VocabularyRoot>;

/// One immutable production table snapshot.
pub type VocabularyModuleTableSnapshot = name_table::ModuleTableSnapshot<VocabularyRoot>;

/// A production seal result.
pub type VocabularySealReceipt = name_table::SealReceipt<VocabularyRoot>;

/// A production rename result.
pub type VocabularyRenameReceipt = name_table::RenameReceipt<VocabularyRoot>;

/// A typed generic-nametable failure over the production roots.
pub type VocabularyNameTableError = name_table::NameTableError<VocabularyRoot>;

/// Which stable root-record surface was decoded.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RootRecordSurface {
    /// A local-socket protocol root record.
    Wire,
    /// A persisted translator archive root record.
    Archive,
}

/// Typed refusal at the root wire/archive boundary.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum RootRecordError {
    /// The record is not the exact fixed size.
    #[error("root record has length {found}; expected {expected}")]
    InvalidLength { expected: usize, found: usize },

    /// The record belongs to another contract.
    #[error("root record has the wrong contract marker")]
    WrongContract { surface: RootRecordSurface },

    /// The record selects an unsupported explicit version.
    #[error("root record version {found} is unsupported")]
    UnsupportedVersion {
        surface: RootRecordSurface,
        found: u16,
    },

    /// The record carries no known production root tag.
    #[error("root record tag {found} is unknown")]
    UnknownRoot { found: u8 },
}

/// Fixed, explicit codec for root records.
pub struct RootRecord;

impl RootRecord {
    /// Encode one root for the local-socket protocol.
    pub const fn encode_wire(root: VocabularyRoot) -> [u8; ROOT_RECORD_LENGTH] {
        encode_record(WIRE_CONTRACT, PROTOCOL_VERSION, root)
    }

    /// Decode one local-socket root record.
    pub fn decode_wire(bytes: &[u8]) -> Result<VocabularyRoot, RootRecordError> {
        decode_record(
            bytes,
            RootRecordSurface::Wire,
            WIRE_CONTRACT,
            PROTOCOL_VERSION,
        )
    }

    /// Encode one root for persisted translator metadata.
    pub const fn encode_archive(root: VocabularyRoot) -> [u8; ROOT_RECORD_LENGTH] {
        encode_record(ARCHIVE_CONTRACT, ARCHIVE_VERSION, root)
    }

    /// Decode one persisted translator root record.
    pub fn decode_archive(bytes: &[u8]) -> Result<VocabularyRoot, RootRecordError> {
        decode_record(
            bytes,
            RootRecordSurface::Archive,
            ARCHIVE_CONTRACT,
            ARCHIVE_VERSION,
        )
    }
}

const fn encode_record(
    contract: [u8; 4],
    version: u16,
    root: VocabularyRoot,
) -> [u8; ROOT_RECORD_LENGTH] {
    let version = version.to_le_bytes();
    [
        contract[0],
        contract[1],
        contract[2],
        contract[3],
        version[0],
        version[1],
        root.tag(),
    ]
}

fn decode_record(
    bytes: &[u8],
    surface: RootRecordSurface,
    contract: [u8; 4],
    supported_version: u16,
) -> Result<VocabularyRoot, RootRecordError> {
    let record: &[u8; ROOT_RECORD_LENGTH] =
        bytes
            .try_into()
            .map_err(|_| RootRecordError::InvalidLength {
                expected: ROOT_RECORD_LENGTH,
                found: bytes.len(),
            })?;

    if record[..4] != contract {
        return Err(RootRecordError::WrongContract { surface });
    }

    let version = u16::from_le_bytes([record[4], record[5]]);
    if version != supported_version {
        return Err(RootRecordError::UnsupportedVersion {
            surface,
            found: version,
        });
    }

    VocabularyRoot::try_from(record[6])
}

/// A submitted production tree violates its root's table policy.
#[derive(Clone, Debug, Eq, PartialEq, thiserror::Error)]
#[error("a {root:?} table was submitted as {submitted:?}; expected {expected:?}")]
pub struct TablePolicyError {
    /// Address-space root whose policy was violated.
    pub root: VocabularyRoot,
    /// Mutability carried by the submitted table.
    pub submitted: TableMutability,
    /// Mutability required below this root.
    pub expected: TableMutability,
}

/// Construct a mutable Universal declaration tree.
pub fn universal_declarations(declarations: Vec<Declaration>) -> RootDeclaration<VocabularyRoot> {
    RootDeclaration::new(
        VocabularyRoot::Universal,
        TableMutability::Mutable,
        declarations,
    )
}

/// Construct an immutable Rust-vocabulary declaration tree.
///
/// The declaration graph can append unseen exact spellings or resolve already
/// allocated ones. It has no operation that can alter, remove, or rebind a
/// stored Rust entry.
pub fn rust_vocabulary_release(declarations: Vec<Declaration>) -> RootDeclaration<VocabularyRoot> {
    RootDeclaration::new(
        VocabularyRoot::Rust,
        TableMutability::Immutable,
        declarations,
    )
}

/// Construct a child module using the production policy of its root.
pub fn module_declaration(
    root: VocabularyRoot,
    spelling: impl Into<name_table::Name>,
    declarations: Vec<Declaration>,
) -> Declaration {
    Declaration::Module(ModuleDeclaration::new(
        spelling,
        root.table_mutability(),
        declarations,
    ))
}

/// Validate that every submitted table follows its production root policy.
///
/// This check is pure. The daemon performs it before staging the generic
/// nametable operation.
pub fn validate_table_policy(
    declaration: &RootDeclaration<VocabularyRoot>,
) -> Result<(), TablePolicyError> {
    let root = *declaration.root();
    let expected = root.table_mutability();
    if declaration.mutability() != expected {
        return Err(TablePolicyError {
            root,
            submitted: declaration.mutability(),
            expected,
        });
    }
    validate_child_policy(root, declaration.declarations())
}

fn validate_child_policy(
    root: VocabularyRoot,
    declarations: &[Declaration],
) -> Result<(), TablePolicyError> {
    let expected = root.table_mutability();
    for declaration in declarations {
        if let Declaration::Module(module) = declaration {
            if module.mutability() != expected {
                return Err(TablePolicyError {
                    root,
                    submitted: module.mutability(),
                    expected,
                });
            }
            validate_child_policy(root, module.declarations())?;
        }
    }
    Ok(())
}

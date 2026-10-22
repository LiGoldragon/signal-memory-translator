use std::cell::Cell;

use name_table::{
    AllocationCursor, EncodedId, LocalEncodedId, ModulePath, Name, NameReference, OperationKey,
    RenameRequest, SnapshotDigest, TableAddress, TableMutability,
};
use signal_frame::{
    ExchangeFrameBody, ExchangeIdentifier, ExchangeLane, FrameError, LaneSequence, NonEmpty, Reply,
    RootCode, SessionEpoch, StreamEventIdentifier, StreamingFrameBody, SubReply,
    SubscriptionTokenInner, VariantCode, WireRoute,
};
use signal_memory_translator::{
    AuthorityCapability, AuthorityOperation, AuthorityReply, AuthorityRequest,
    AuthorityRequestDigest, AuthorityRole, AuthorizationClaim, CONTRACT_BINDING, CommittedReceipt,
    CurrentAuthority, DatabaseMarker, DeclarationNode, DtoConversionError, ExpectedTableGeneration,
    NoWriteFailure, ObservedHead, ObservedSnapshot, PostCommitEvent, PrincipalId, ReadOperation,
    ReferencePath, Rename, RenameCommitReceipt, RustVocabularyCommitReceipt, RustVocabularyRelease,
    RustVocabularyVersion, SealCommitReceipt, SealUniversal, TranslatorFrame, VocabularyEncodedId,
    VocabularyNameTable, VocabularyRoot, WritePrecondition,
};

const ROUTE: WireRoute = WireRoute::new(RootCode::new(0x21), VariantCode::new(0x34));

fn key(byte: u8) -> OperationKey {
    OperationKey::new([byte; 32])
}

fn auth(byte: u8, role: AuthorityRole, capability: AuthorityCapability) -> AuthorizationClaim {
    AuthorizationClaim {
        principal: PrincipalId::new([byte; 32]),
        role,
        capability,
    }
}

fn expected(sequence: u64) -> WritePrecondition {
    WritePrecondition {
        database_marker: DatabaseMarker::new(sequence, sequence + 1),
        table_generations: vec![ExpectedTableGeneration {
            address: TableAddress::root(VocabularyRoot::Universal),
            generation: sequence + 2,
        }],
    }
}

fn member(spelling: &str) -> DeclarationNode {
    DeclarationNode::Member(Name::new(spelling))
}

fn module(spelling: &str, declarations: Vec<DeclarationNode>) -> DeclarationNode {
    DeclarationNode::Module {
        spelling: Name::new(spelling),
        declarations,
    }
}

fn reference(root: VocabularyRoot, modules: &[&str], spelling: &str) -> ReferencePath {
    ReferencePath {
        root,
        modules: modules.iter().copied().map(Name::new).collect(),
        spelling: Name::new(spelling),
    }
}

fn seal_request(operation_key: OperationKey) -> SealUniversal {
    SealUniversal {
        operation_key,
        expected: expected(10),
        declarations: vec![
            member("Integer"),
            module(
                "billing",
                vec![member("Invoice"), module("line", vec![member("Amount")])],
            ),
        ],
        references: vec![
            reference(VocabularyRoot::Universal, &[], "Integer"),
            reference(VocabularyRoot::Universal, &["billing"], "Invoice"),
        ],
    }
}

fn rust_release(operation_key: OperationKey) -> RustVocabularyRelease {
    RustVocabularyRelease {
        operation_key,
        version: RustVocabularyVersion::new(20260729),
        expected: expected(20),
        declarations: vec![module(
            "std",
            vec![module("primitive", vec![member("u64"), member("bool")])],
        )],
    }
}

fn authority_request(operation: AuthorityOperation) -> AuthorityRequest {
    let (role, capability) = match operation {
        AuthorityOperation::SealUniversal(_) => (
            AuthorityRole::UniversalAuthor,
            AuthorityCapability::SealUniversal,
        ),
        AuthorityOperation::Rename(_) => (
            AuthorityRole::UniversalMaintainer,
            AuthorityCapability::Rename,
        ),
        AuthorityOperation::PublishRustVocabulary(_) => (
            AuthorityRole::RustVocabularyPublisher,
            AuthorityCapability::PublishRustVocabulary,
        ),
        AuthorityOperation::Read(_) => (AuthorityRole::Reader, AuthorityCapability::Read),
    };
    AuthorityRequest {
        authorization: auth(7, role, capability),
        operation,
    }
}

fn exchange(sequence: u64) -> ExchangeIdentifier {
    ExchangeIdentifier::new(
        SessionEpoch::new(3),
        ExchangeLane::Connector,
        LaneSequence::new(sequence),
    )
}

fn round_trip_request(request: AuthorityRequest) -> AuthorityRequest {
    let frame = TranslatorFrame::new(
        ROUTE,
        StreamingFrameBody::Request {
            exchange: exchange(1),
            request: signal_frame::Request::from_payload(request),
        },
    );
    let bytes = frame.encode_length_prefixed().expect("encode request");
    let decoded = TranslatorFrame::decode_length_prefixed(&bytes).expect("decode request");
    match decoded.into_body() {
        StreamingFrameBody::Request { request, .. } => {
            assert!(request.payloads.tail().is_empty());
            request.payloads.head().clone()
        }
        body => panic!("expected request body, got {body:?}"),
    }
}

fn round_trip_reply(reply: AuthorityReply) -> AuthorityReply {
    let frame = TranslatorFrame::new(
        ROUTE,
        StreamingFrameBody::Reply {
            exchange: exchange(2),
            reply: Reply::committed(NonEmpty::single(SubReply::Ok(reply))),
        },
    );
    let bytes = frame.encode_length_prefixed().expect("encode reply");
    let decoded = TranslatorFrame::decode_length_prefixed(&bytes).expect("decode reply");
    match decoded.into_body() {
        StreamingFrameBody::Reply {
            reply: Reply::Accepted { per_operation, .. },
            ..
        } => match per_operation.head() {
            SubReply::Ok(reply) => reply.clone(),
            reply => panic!("expected successful subreply, got {reply:?}"),
        },
        body => panic!("expected reply body, got {body:?}"),
    }
}

fn round_trip_event(event: PostCommitEvent) -> PostCommitEvent {
    let frame = TranslatorFrame::new(
        ROUTE,
        StreamingFrameBody::SubscriptionEvent {
            event_identifier: StreamEventIdentifier::acceptor(
                SessionEpoch::new(3),
                LaneSequence::new(4),
            ),
            token: SubscriptionTokenInner::new(5),
            event,
        },
    );
    let bytes = frame.encode_length_prefixed().expect("encode event");
    let decoded = TranslatorFrame::decode_length_prefixed(&bytes).expect("decode event");
    match decoded.into_body() {
        StreamingFrameBody::SubscriptionEvent { event, .. } => event,
        body => panic!("expected event body, got {body:?}"),
    }
}

struct Fixture {
    names: VocabularyNameTable,
    universal: SealCommitReceipt,
    rename: RenameCommitReceipt,
    rust: RustVocabularyCommitReceipt,
}

fn fixture() -> Fixture {
    let mut names = VocabularyNameTable::new();
    let universal = seal_request(key(1));
    let universal_receipt = names.seal(universal.lower()).expect("Universal seal");
    let integer = universal_receipt
        .declarations()
        .iter()
        .find(|resolved| resolved.path().spelling().as_str() == "Integer")
        .expect("Integer")
        .encoded_id()
        .clone();
    let rename_receipt = names
        .rename(RenameRequest::new(key(2), integer, "WholeNumber"))
        .expect("rename");
    let rust = rust_release(key(3));
    let rust_receipt = names.seal(rust.lower()).expect("Rust release");
    Fixture {
        names,
        universal: SealCommitReceipt {
            request_digest: AuthorityRequestDigest::new([11; 32]),
            name_table: universal_receipt,
            database_marker: DatabaseMarker::new(1, 1),
        },
        rename: RenameCommitReceipt {
            request_digest: AuthorityRequestDigest::new([12; 32]),
            name_table: rename_receipt,
            database_marker: DatabaseMarker::new(2, 2),
        },
        rust: RustVocabularyCommitReceipt {
            version: rust.version,
            request_digest: AuthorityRequestDigest::new([13; 32]),
            name_table: rust_receipt,
            database_marker: DatabaseMarker::new(3, 3),
        },
    }
}

#[test]
fn contract_binding_is_exact_and_validated_before_body_decode() {
    let request = authority_request(AuthorityOperation::Read(ReadOperation::Current));
    let frame = TranslatorFrame::new(
        ROUTE,
        StreamingFrameBody::Request {
            exchange: exchange(1),
            request: signal_frame::Request::from_payload(request),
        },
    );
    assert_eq!(CONTRACT_BINDING.contract().value(), 4);
    assert_eq!(CONTRACT_BINDING.revision().value(), 1);
    assert_eq!(
        frame.short_header().to_le_bytes(),
        [4, 0, 0, 0, 1, 0, 0x34, 0x21]
    );

    let original = frame.encode().expect("encode current frame");
    for (mut bytes, expected) in [
        {
            let mut bytes = original.clone();
            bytes[..4].copy_from_slice(&3_u32.to_le_bytes());
            (
                bytes,
                FrameError::ContractMismatch {
                    expected: CONTRACT_BINDING.contract(),
                    found: signal_frame::ContractId::try_new(3).unwrap(),
                },
            )
        },
        {
            let mut bytes = original.clone();
            bytes[4..6].copy_from_slice(&2_u16.to_le_bytes());
            (
                bytes,
                FrameError::UnsupportedWireRevision {
                    contract: CONTRACT_BINDING.contract(),
                    expected: CONTRACT_BINDING.revision(),
                    found: signal_frame::WireRevision::try_new(2).unwrap(),
                },
            )
        },
    ] {
        bytes.truncate(signal_frame::SHORT_HEADER_BYTE_COUNT);
        let invoked = Cell::new(false);
        let error = CONTRACT_BINDING
            .decode_archive_with(&bytes, |_| {
                invoked.set(true);
                Ok(())
            })
            .expect_err("wrong binding");
        assert!(!invoked.get());
        assert_eq!(error.to_string(), expected.to_string());
    }

    let mut legacy = original;
    legacy[..8].fill(0);
    legacy.truncate(signal_frame::SHORT_HEADER_BYTE_COUNT);
    assert!(matches!(
        TranslatorFrame::decode(&legacy),
        Err(FrameError::UnboundHeader)
    ));
}

#[test]
fn all_operation_variants_round_trip_and_lower_without_mutability_input() {
    let seal = seal_request(key(10));
    assert_eq!(seal.expected, expected(10));
    let lowered = seal.lower();
    assert_eq!(lowered.roots().len(), 1);
    assert_eq!(lowered.roots()[0].root(), &VocabularyRoot::Universal);
    assert_eq!(lowered.roots()[0].mutability(), TableMutability::Mutable);
    assert_eq!(
        SealUniversal::from_name_table(&lowered, seal.expected.clone()).unwrap(),
        seal
    );
    assert!(matches!(
        RustVocabularyRelease::from_name_table(
            &lowered,
            RustVocabularyVersion::new(1),
            expected(0),
        ),
        Err(DtoConversionError::WrongRoot {
            expected: VocabularyRoot::Rust,
            found: VocabularyRoot::Universal,
        })
    ));
    let DeclarationNode::Module { declarations, .. } = &seal.declarations[1] else {
        panic!("expected nested module");
    };
    assert_eq!(declarations.len(), 2);

    let rust = rust_release(key(11));
    let lowered_rust = rust.lower();
    assert_eq!(lowered_rust.roots()[0].root(), &VocabularyRoot::Rust);
    assert_eq!(
        lowered_rust.roots()[0].mutability(),
        TableMutability::Immutable
    );
    assert_eq!(
        RustVocabularyRelease::from_name_table(&lowered_rust, rust.version, rust.expected.clone(),)
            .unwrap(),
        rust
    );

    let rename = Rename {
        operation_key: key(12),
        expected: expected(30),
        target: EncodedId::new(
            VocabularyRoot::Universal,
            vec![LocalEncodedId::new(4), LocalEncodedId::new(8)],
        )
        .unwrap(),
        new_spelling: Name::new("renamed"),
    };
    assert_eq!(rename.lower().target(), &rename.target);
    assert_eq!(
        Rename::from_name_table(&rename.lower(), rename.expected.clone()),
        rename
    );

    let operations = vec![
        AuthorityOperation::SealUniversal(seal),
        AuthorityOperation::Rename(rename),
        AuthorityOperation::PublishRustVocabulary(rust),
        AuthorityOperation::Read(ReadOperation::Current),
        AuthorityOperation::Read(ReadOperation::Head {
            address: TableAddress::root(VocabularyRoot::Universal),
        }),
        AuthorityOperation::Read(ReadOperation::CurrentSnapshot {
            address: TableAddress::root(VocabularyRoot::Rust),
        }),
        AuthorityOperation::Read(ReadOperation::CommittedReceipt {
            operation_key: key(13),
        }),
    ];
    for operation in operations {
        let request = authority_request(operation);
        assert_eq!(round_trip_request(request.clone()), request);
    }
}

#[test]
fn declaration_order_is_not_identity_or_digest_order() {
    let left = SealUniversal {
        operation_key: key(20),
        expected: expected(0),
        declarations: vec![
            member("zeta"),
            module("module", vec![member("third"), member("first")]),
            member("alpha"),
        ],
        references: vec![],
    };
    let right = SealUniversal {
        operation_key: key(21),
        expected: expected(0),
        declarations: vec![
            member("alpha"),
            member("zeta"),
            module("module", vec![member("first"), member("third")]),
        ],
        references: vec![],
    };
    let mut left_names = VocabularyNameTable::new();
    let mut right_names = VocabularyNameTable::new();
    let left_receipt = left_names.seal(left.lower()).expect("left");
    let right_receipt = right_names.seal(right.lower()).expect("right");

    assert_eq!(
        left_receipt.request_digest(),
        right_receipt.request_digest()
    );
    assert_eq!(left_receipt.declarations(), right_receipt.declarations());
    assert_eq!(
        left_receipt.changed_tables(),
        right_receipt.changed_tables()
    );
    assert_eq!(
        left.canonical_request_digest().expect("left digest"),
        right.canonical_request_digest().expect("right digest")
    );
}

#[test]
fn producer_digest_and_database_successor_bind_receipt_materialization() {
    let planned = seal_request(key(22));
    let mut changed = planned.clone();
    changed.references.push(reference(
        VocabularyRoot::Universal,
        &["billing", "line"],
        "Amount",
    ));

    assert_ne!(
        planned
            .canonical_request_digest()
            .expect("planned canonical digest"),
        changed
            .canonical_request_digest()
            .expect("changed canonical digest")
    );
    assert_eq!(
        DatabaseMarker::new(41, 73).checked_successor(),
        Some(DatabaseMarker::new(42, 74))
    );
    assert_eq!(DatabaseMarker::new(u64::MAX, 73).checked_successor(), None);
    assert_eq!(DatabaseMarker::new(41, u64::MAX).checked_successor(), None);
}

#[test]
fn redefinition_refuses_seal_separately_from_sibling_homonyms() {
    let duplicate = SealUniversal {
        operation_key: key(30),
        expected: expected(0),
        declarations: vec![module("one", vec![member("same"), member("same")])],
        references: vec![],
    };
    let mut duplicate_names = VocabularyNameTable::new();
    let error = duplicate_names
        .seal(duplicate.lower())
        .expect_err("same-table redefinition");
    assert!(matches!(
        NoWriteFailure::from(error),
        NoWriteFailure::Redefinition { spelling, .. } if spelling == Name::new("same")
    ));
    assert_eq!(duplicate_names.revision().value(), 0);

    let siblings = SealUniversal {
        operation_key: key(31),
        expected: expected(0),
        declarations: vec![
            module("left", vec![member("same")]),
            module("right", vec![member("same")]),
        ],
        references: vec![],
    };
    let mut sibling_names = VocabularyNameTable::new();
    let receipt = sibling_names
        .seal(siblings.lower())
        .expect("sibling homonyms are valid");
    let same = receipt
        .declarations()
        .iter()
        .filter(|resolved| resolved.path().spelling().as_str() == "same")
        .map(|resolved| resolved.encoded_id().clone())
        .collect::<Vec<_>>();
    assert_eq!(same.len(), 2);
    assert_ne!(same[0], same[1]);
    assert_ne!(same[0].owning_table(), same[1].owning_table());
}

#[test]
fn declarations_allocate_but_cross_root_references_only_resolve() {
    let mut names = VocabularyNameTable::new();
    names
        .seal(seal_request(key(40)).lower())
        .expect("Universal");
    names
        .seal(
            RustVocabularyRelease {
                operation_key: key(41),
                version: RustVocabularyVersion::new(1),
                expected: expected(0),
                declarations: Vec::new(),
            }
            .lower(),
        )
        .expect("empty Rust root");
    let rust_before = names
        .head(&TableAddress::root(VocabularyRoot::Rust))
        .expect("Rust head")
        .clone();

    let unresolved = SealUniversal {
        operation_key: key(42),
        expected: expected(0),
        declarations: vec![],
        references: vec![reference(VocabularyRoot::Rust, &[], "Integer")],
    };
    let failure = NoWriteFailure::from(
        names
            .seal(unresolved.lower())
            .expect_err("must not fall back to Universal"),
    );
    assert!(matches!(
        failure,
        NoWriteFailure::UnresolvedReference { reference }
            if reference.path().table().root() == &VocabularyRoot::Rust
    ));
    assert_eq!(
        names
            .head(&TableAddress::root(VocabularyRoot::Rust))
            .expect("Rust head after refusal"),
        &rust_before
    );
}

#[test]
fn text_edits_remint_while_operational_rename_preserves_full_chains() {
    let mut names = VocabularyNameTable::new();
    let first = SealUniversal {
        operation_key: key(50),
        expected: expected(0),
        declarations: vec![module("old_module", vec![member("Thing")])],
        references: vec![],
    };
    let first_receipt = names.seal(first.lower()).expect("first text");
    let old_module = first_receipt
        .declarations()
        .iter()
        .find(|resolved| resolved.path().spelling().as_str() == "old_module")
        .unwrap()
        .encoded_id()
        .clone();
    let old_thing = first_receipt
        .declarations()
        .iter()
        .find(|resolved| resolved.path().spelling().as_str() == "Thing")
        .unwrap()
        .encoded_id()
        .clone();

    let edited = SealUniversal {
        operation_key: key(51),
        expected: expected(0),
        declarations: vec![module("new_module", vec![member("Thing")])],
        references: vec![],
    };
    let edited_receipt = names.seal(edited.lower()).expect("edited text");
    let new_module = edited_receipt
        .declarations()
        .iter()
        .find(|resolved| resolved.path().spelling().as_str() == "new_module")
        .unwrap()
        .encoded_id()
        .clone();
    let new_thing = edited_receipt
        .declarations()
        .iter()
        .find(|resolved| {
            resolved.path().table().modules() == [Name::new("new_module")]
                && resolved.path().spelling().as_str() == "Thing"
        })
        .unwrap()
        .encoded_id()
        .clone();
    assert_ne!(old_module, new_module);
    assert_ne!(old_thing, new_thing);
    assert_eq!(names.resolve(&old_module).unwrap().as_str(), "old_module");
    assert_eq!(names.resolve(&old_thing).unwrap().as_str(), "Thing");

    let descendant_chain = old_thing.clone();
    let renamed = names
        .rename(RenameRequest::new(
            key(52),
            old_module.clone(),
            "kept_module",
        ))
        .expect("operational module rename");
    assert_eq!(renamed.target(), &old_module);
    assert_eq!(names.resolve(&old_module).unwrap().as_str(), "kept_module");
    assert_eq!(names.resolve(&descendant_chain).unwrap().as_str(), "Thing");
}

#[test]
fn immutable_refusal_precedes_entry_lookup_and_rust_release_only_appends() {
    let mut names = VocabularyNameTable::new();
    let first = names
        .seal(rust_release(key(60)).lower())
        .expect("first Rust release");
    let u64_id = first
        .declarations()
        .iter()
        .find(|resolved| resolved.path().spelling().as_str() == "u64")
        .unwrap()
        .encoded_id()
        .clone();
    let second = names
        .seal(
            RustVocabularyRelease {
                operation_key: key(61),
                version: RustVocabularyVersion::new(20260730),
                expected: expected(0),
                declarations: vec![module(
                    "std",
                    vec![module("primitive", vec![member("u64"), member("usize")])],
                )],
            }
            .lower(),
        )
        .expect("append Rust release");
    assert_eq!(
        second
            .declarations()
            .iter()
            .find(|resolved| resolved.path().spelling().as_str() == "u64")
            .unwrap()
            .encoded_id(),
        &u64_id
    );

    let mut missing_chain = u64_id.chain().to_vec();
    *missing_chain.last_mut().unwrap() = LocalEncodedId::new(u16::MAX);
    let missing = EncodedId::new(VocabularyRoot::Rust, missing_chain).unwrap();
    let failure = NoWriteFailure::from(
        names
            .rename(RenameRequest::new(key(62), missing, "never"))
            .expect_err("immutable before lookup"),
    );
    assert!(matches!(failure, NoWriteFailure::ImmutableTable { .. }));
}

#[test]
fn current_head_current_snapshot_historical_and_receipt_replies_round_trip() {
    let fixture = fixture();
    let address = TableAddress::root(VocabularyRoot::Universal);
    let head = fixture.names.head(&address).unwrap().clone();
    let snapshot = fixture.names.current_snapshot(&address).unwrap().clone();

    let reads = [
        AuthorityReply::Current(CurrentAuthority {
            database_marker: DatabaseMarker::new(3, 3),
            name_table_revision: fixture.names.revision(),
        }),
        AuthorityReply::Head(ObservedHead {
            database_marker: DatabaseMarker::new(3, 3),
            head,
        }),
        AuthorityReply::Snapshot(ObservedSnapshot {
            database_marker: DatabaseMarker::new(3, 3),
            snapshot: snapshot.clone(),
        }),
        AuthorityReply::Receipt(CommittedReceipt::SealUniversal(fixture.universal.clone())),
    ];
    for reply in reads {
        assert_eq!(round_trip_reply(reply.clone()), reply);
    }

    let request = authority_request(AuthorityOperation::Read(
        ReadOperation::HistoricalSnapshot {
            address,
            snapshot: snapshot.digest(),
        },
    ));
    assert_eq!(round_trip_request(request.clone()), request);
}

#[test]
fn committed_receipts_and_post_commit_events_round_trip_exactly() {
    let fixture = fixture();
    let receipts = [
        CommittedReceipt::SealUniversal(fixture.universal.clone()),
        CommittedReceipt::Rename(fixture.rename.clone()),
        CommittedReceipt::PublishRustVocabulary(fixture.rust.clone()),
    ];
    for receipt in receipts {
        let reply = AuthorityReply::Committed(receipt.clone());
        assert_eq!(round_trip_reply(reply.clone()), reply);
        assert_eq!(
            receipt.operation_key(),
            match &receipt {
                CommittedReceipt::SealUniversal(value) => value.name_table.operation_key(),
                CommittedReceipt::Rename(value) => value.name_table.operation_key(),
                CommittedReceipt::PublishRustVocabulary(value) => value.name_table.operation_key(),
            }
        );
    }

    let events = [
        PostCommitEvent::UniversalSealed(fixture.universal),
        PostCommitEvent::Renamed(fixture.rename),
        PostCommitEvent::RustVocabularyPublished(fixture.rust),
    ];
    for event in events {
        assert_eq!(round_trip_event(event.clone()), event);
    }
}

#[test]
fn all_no_write_failure_variants_are_rkyv_frame_payloads() {
    let fixture = fixture();
    let address = TableAddress::root(VocabularyRoot::Universal);
    let snapshot = fixture.names.current_snapshot(&address).unwrap();
    let encoded = fixture.rename.name_table.target().clone();
    let module_path = ModulePath::new(VocabularyRoot::Universal, vec![Name::new("m")]);
    let reference = NameReference::new(ModulePath::new(VocabularyRoot::Rust, vec![]), "missing");
    let revision = fixture.names.revision();
    let rust_version = RustVocabularyVersion::new(8);
    let failures = vec![
        NoWriteFailure::AuthorizationDenied {
            principal: PrincipalId::new([1; 32]),
            role: AuthorityRole::UniversalMaintainer,
            capability: AuthorityCapability::Rename,
        },
        NoWriteFailure::StaleDatabaseMarker {
            expected: DatabaseMarker::new(1, 1),
            actual: DatabaseMarker::new(2, 2),
        },
        NoWriteFailure::StaleTableGeneration {
            address: address.clone(),
            expected: 1,
            actual: 2,
        },
        NoWriteFailure::DuplicateExpectedTable {
            address: address.clone(),
        },
        NoWriteFailure::DuplicateRootDeclaration {
            root: VocabularyRoot::Universal,
        },
        NoWriteFailure::Redefinition {
            table: module_path,
            spelling: Name::new("same"),
        },
        NoWriteFailure::TableMutabilityMismatch {
            address: address.clone(),
            existing: TableMutability::Mutable,
            submitted: TableMutability::Immutable,
        },
        NoWriteFailure::UnknownTable {
            address: address.clone(),
        },
        NoWriteFailure::UnresolvedReference { reference },
        NoWriteFailure::UnknownEncodedId {
            encoded_id: encoded.clone(),
        },
        NoWriteFailure::ImmutableTable {
            address: address.clone(),
        },
        NoWriteFailure::SpellingCollision {
            address: address.clone(),
            spelling: Name::new("same"),
            existing: LocalEncodedId::new(3),
        },
        NoWriteFailure::TableCapacity {
            address: address.clone(),
        },
        NoWriteFailure::TableGenerationExhausted {
            address: address.clone(),
        },
        NoWriteFailure::StateRevisionExhausted,
        NoWriteFailure::IdempotencyConflict {
            operation_key: key(70),
        },
        NoWriteFailure::StaleStage {
            expected: revision,
            actual: revision,
        },
        NoWriteFailure::UnknownCommittedReceipt {
            operation_key: key(71),
        },
        NoWriteFailure::StaleRustVocabularyVersion {
            submitted: RustVocabularyVersion::new(7),
            current: rust_version,
        },
        NoWriteFailure::RustVocabularyVersionConflict {
            version: rust_version,
        },
        NoWriteFailure::EmptyStoredEncodedId,
        NoWriteFailure::InconsistentTableAncestry {
            address: address.clone(),
        },
        NoWriteFailure::InconsistentTableHead {
            address: address.clone(),
        },
        NoWriteFailure::DuplicateSnapshotSpelling {
            address: address.clone(),
            spelling: Name::new("same"),
        },
        NoWriteFailure::SnapshotCapacity {
            address: address.clone(),
        },
        NoWriteFailure::SnapshotIntegrityMismatch {
            digest: snapshot.digest(),
        },
        NoWriteFailure::DuplicateArchiveRecord,
        NoWriteFailure::InconsistentReceipt {
            operation_key: key(72),
        },
        NoWriteFailure::InvalidArchiveEnvelope,
        NoWriteFailure::ArchiveIntegrityMismatch,
        NoWriteFailure::UnsupportedArchiveVersion { found: 2 },
        NoWriteFailure::SerializationFailed {
            message: "serialize".to_owned(),
        },
        NoWriteFailure::DeserializationFailed {
            message: "deserialize".to_owned(),
        },
        NoWriteFailure::CommitFailed {
            message: "commit".to_owned(),
        },
        NoWriteFailure::AuthorityUnavailable,
    ];

    for failure in failures {
        let reply = AuthorityReply::Rejected(failure);
        assert_eq!(round_trip_reply(reply.clone()), reply);
    }
}

#[test]
fn full_encoded_id_chains_round_trip_for_both_roots_and_boundary_values() {
    for root in [VocabularyRoot::Universal, VocabularyRoot::Rust] {
        assert!(VocabularyEncodedId::new(root, Vec::new()).is_err());
        for chain in [
            vec![LocalEncodedId::new(0)],
            vec![LocalEncodedId::new(u16::MAX)],
            vec![
                LocalEncodedId::new(0),
                LocalEncodedId::new(1),
                LocalEncodedId::new(255),
                LocalEncodedId::new(u16::MAX),
            ],
        ] {
            let encoded = VocabularyEncodedId::new(root, chain).unwrap();
            let request = authority_request(AuthorityOperation::Rename(Rename {
                operation_key: key(root.tag()),
                expected: WritePrecondition {
                    database_marker: DatabaseMarker::GENESIS,
                    table_generations: vec![],
                },
                target: encoded,
                new_spelling: Name::new("exact"),
            }));
            assert_eq!(round_trip_request(request.clone()), request);
        }
    }
}

#[test]
fn correct_binding_with_corrupt_archive_is_a_body_error() {
    let request = authority_request(AuthorityOperation::Read(ReadOperation::Current));
    let frame = TranslatorFrame::new(
        ROUTE,
        StreamingFrameBody::Request {
            exchange: exchange(80),
            request: signal_frame::Request::from_payload(request),
        },
    );
    let mut bytes = frame.encode().unwrap();
    bytes.truncate(signal_frame::SHORT_HEADER_BYTE_COUNT + 1);
    assert!(matches!(
        TranslatorFrame::decode(&bytes),
        Err(FrameError::ArchiveDeserialize(_))
    ));
}

#[test]
fn archived_unknown_root_discriminants_are_rejected() {
    let mut bytes =
        rkyv::to_bytes::<rkyv::rancor::Error>(&VocabularyRoot::Universal).expect("archive root");
    assert_eq!(bytes.len(), 1);
    bytes[0] = 99;
    assert!(rkyv::from_bytes::<VocabularyRoot, rkyv::rancor::Error>(&bytes).is_err());
}

#[test]
fn generic_legacy_exchange_body_cannot_decode_as_the_bound_stream_contract() {
    let legacy_body: ExchangeFrameBody<AuthorityRequest, AuthorityReply> =
        ExchangeFrameBody::Request {
            exchange: exchange(90),
            request: signal_frame::Request::from_payload(authority_request(
                AuthorityOperation::Read(ReadOperation::Current),
            )),
        };
    let archived = rkyv::to_bytes::<rkyv::rancor::Error>(&legacy_body).unwrap();
    let mut bytes = vec![0; signal_frame::SHORT_HEADER_BYTE_COUNT];
    bytes.extend_from_slice(&archived);
    assert!(matches!(
        TranslatorFrame::decode(&bytes),
        Err(FrameError::UnboundHeader)
    ));
}

#[test]
fn production_policy_heads_expose_the_expected_cursor_and_mutability() {
    let fixture = fixture();
    let universal = fixture
        .names
        .head(&TableAddress::root(VocabularyRoot::Universal))
        .unwrap();
    let rust = fixture
        .names
        .head(&TableAddress::root(VocabularyRoot::Rust))
        .unwrap();
    assert_eq!(universal.mutability(), TableMutability::Mutable);
    assert_eq!(rust.mutability(), TableMutability::Immutable);
    assert!(!matches!(
        universal.allocation_cursor(),
        AllocationCursor::Exhausted
    ));
    assert!(!matches!(
        rust.allocation_cursor(),
        AllocationCursor::Exhausted
    ));
}

fn _snapshot_digest_is_contract_typed(_: SnapshotDigest) {}

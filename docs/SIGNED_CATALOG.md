# Signed catalog protocol and delivery

The app accepts only built-in immutable pins or catalog envelopes meeting the
release's pinned Ed25519 quorum. The remote document cannot supply trust keys.
The endpoint is `https://raw.githubusercontent.com/snapetech-org/MilkRust/catalog/visualizer-catalog.json`.

Envelope v1 has `schemaVersion`, base64 `payload`, and `signatures` containing
`keyId` and base64 `signature`. Sign the exact bytes `MilkRust.catalog.v1\n`
followed by the decoded payload. Signatures are Ed25519, not prehashed Ed25519.
Payload v1 carries a positive monotonic `sequence`, UTC `issuedAt`/`expiresAt`,
`entries`, and cumulative `revoked` identities (`id@version` or `id@*`). Maximum
lifetime is 31 days; the publisher defaults to seven days. A new sequence is
needed for renewal, even if the pack list is unchanged. One current version per
pack is allowed. Archives pin a full 40-character MilkRust Git commit and SHA256.

`tools/signed_catalog.py` prepares metadata from immutable archives, rechecks
integrity and per-file attribution, signs, verifies, and publishes using a
non-force branch update. A racing update fails instead of losing history. The
publisher reads the previous authenticated envelope, refuses sequence rollback,
and requires revocations to remain present. Signing uses provisioned OpenSSL 3;
there is no downloaded crypto package or execution of preset content.

## Release provisioning

The source-side preparation is now split from credential provisioning. Run
`python3 tools/catalog_release_preflight.py` or dispatch **Catalog release
preflight (no credentials)** from `main` to inspect the checked-in trust root,
publisher workflow and catalog shape. The report is deliberately keyless and
offline: it does not read environment variables, GitHub secrets, API
credentials or signing keys. Its expected current result is
`prepared-blocked-on-provisioning`, because `content/catalog-trust.json` still
has no production public keys. The workflow uploads only that readiness report
and has repository read permission.

1. Generate Ed25519 keys in the organization's approved key custody system.
   Back them up and limit access to the protected catalog environment. This
   repository intentionally contains no production private key or invented key.
2. Export only Ed25519 public keys from that custody system. Use
   `tools/catalog_trust.py --public-key ID=PUBLIC.pem --threshold N
   --minimum-sequence S --output content/catalog-trust.json` (repeat
   `--public-key` for each independent key). This accepts public SPKI PEM/DER,
   rejects private-key input and emits `status: active`; it does not provision
   custody or GitHub settings. Review and commit the resulting public root.
   Configure the corresponding private PEM map as
   `MILKRUST_CATALOG_PRIVATE_KEYS` in the protected `visualizer-catalog`
   environment. Confirm compatible private ephemeral JIT runner capacity and
   broker routing for `catalog-publish` before dispatch.
3. Pin that exact public file and immutable MilkRust revision in PlezyNG with
   `scripts/visualizer/configure_catalog_trust.py --input PUBLIC_ROOT.json
   --source-revision COMMIT --sha256 REVIEWED_ROOT_SHA256`. Commit its generated
   Dart source in an app release. Every platform compiles the same reviewed
   root; build-environment defines no longer replace it. Empty keys explicitly
   disable remote updates; included packs and local imports continue to work.
   The release sequence floor also protects first install, before checkpoints.
4. Merge reviewed immutable archives/catalog metadata and the required
   [qualification evidence](../content/qualification/README.md), then dispatch **Publish
   signed visualizer catalog** from protected `main` with a new sequence. The
   first publication explicitly uses `bootstrap`. Verify the public endpoint
   through the app, including cancellation, expiry, rollback and offline use.
5. Renew before expiry. Signing is manual until custody and runner operations
   have been exercised; no unattended production job is enabled by this PR.

The preflight does not create a public trust root, pin it in the app, provision
an environment, configure any credential, test runner capacity, sign an
envelope or publish a catalog. Those require the separately approved
custody/platform operations above.

For rotation, ship overlapping public keys first, publish signatures satisfying
both old and new supported app quorums during migration, then retire old keys in
a new app release. If an old signing key is compromised, a catalog signed only
by a newly announced key cannot establish trust; distribute an app trust-root
update. For content withdrawal, add a cumulative revocation and remove the entry,
publish a higher sequence and preserve notices/history. Installed user content
is not silently deleted. Wildcard revocations permanently retire the ID; restoring
content uses a new ID. Exact revocations require a new version.

The app checkpoints authenticated sequence/digest pairs before activating a new
catalog, retains the latest eight, and keeps installed pack receipts separately.
Invalid signatures/network failure retain the last valid catalog. Expiry denies
new remote installation but does not hide installed files. Downloads recheck
current authorization immediately before activation. No fetch occurs merely
because music starts playing. Local storage tampering, loss of all checkpoints,
and maliciously changing the device clock are outside this network rollback model.
The publisher authenticates old history even below a newly raised release floor,
but refuses to sign/publish a new envelope below that floor. Keep overlapping
verification keys until the old publication has been migrated.

## Current evidence

Source and regression cases only. Production keys, protected environment,
private JIT execution, signed endpoint publication and installed-app verification
are not provisioned or demonstrated by these files. Do not advertise signed
catalog delivery until those steps have evidence. Confidence: moderate for
protocol/source coverage; operational delivery remains unknown.

# Integration with Stellar Explorer Metadata
## Task #213 — DevEx & Ecosystem Tooling

This task improves how the MarketX contract appears on Stellar explorers (including Stellar.expert) by embedding contract metadata in the WASM and documenting optional off-chain metadata publication.

---

## 1) What was integrated

### A. On-chain contract metadata annotations

The contract now includes Soroban `#[contractmeta]` entries for:
- Name
- Description
- Homepage URL
- Repository URL
- Source code URL
- Build/version tag

These values are bundled into the contract artifact and can be consumed by explorer tooling that reads Soroban metadata sections.

### B. Off-chain explorer profile template

A companion metadata template is provided at:
- `docs/stellar-expert-metadata.json`

This gives the ops/devrel team a canonical payload for explorer profiles, registry submissions, or internal metadata services.

---

## 2) Explorer-facing recommendations

1. Keep `version` aligned with release tags.
2. Keep homepage + repo links stable and publicly accessible.
3. Use a deterministic icon URL (CDN/IPFS with immutable hash when possible).
4. Update metadata alongside every production contract upgrade.

---

## 3) Verification flow

After deployment, verify that metadata is discoverable:

1. Build and deploy the updated WASM.
2. Open the contract page on Stellar.expert.
3. Confirm readable project identity fields (name/description/links).
4. Cross-check values against `docs/stellar-expert-metadata.json`.

---

## 4) Operational runbook snippet

- During release prep:
  - bump `version` metadata value
  - verify repo/homepage links
- During deploy:
  - publish the new WASM
- Post-deploy:
  - validate explorer rendering
  - publish release note referencing new contract ID

---

## 5) Outcome

MarketX now exposes cleaner project identity metadata for ecosystem tooling and contract explorers, improving discoverability and trust for non-technical users.

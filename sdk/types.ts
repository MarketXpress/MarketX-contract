/**
 * MarketX Contract — TypeScript Type Definitions
 *
 * Auto-generated from Rust contract types for frontend integration.
 * These types mirror the Soroban contract types defined in contracts/marketx/src/types.rs
 *
 * Usage:
 *   import { Escrow, EscrowStatus, EscrowItem } from './types';
 */

export const CONTRACT_VERSION = '1.0.0';

export interface ContractVersion {
  major: number;
  minor: number;
  patch: number;
}

export const MAX_METADATA_SIZE = 1024;
export const MAX_DESCRIPTION_SIZE = 256;
export const CURRENT_SCHEMA_VERSION = 1;
export const MAX_TRACKING_ID_SIZE = 128;
export const MAX_EVIDENCE_HASH_SIZE = 128;
export const MAX_ITEMS_PER_ESCROW = 50;
export const UNFUNDED_EXPIRY_LEDGERS = 120960;
export const DEFAULT_EVIDENCE_WINDOW_LEDGERS = 34560;
export const APPEAL_WINDOW_LEDGERS = 17280;
export const DEFAULT_MEDIATION_WINDOW_LEDGERS = 34560;

export type EscrowStatus =
  | 'Pending'
  | 'Funded'
  | 'Released'
  | 'Refunded'
  | 'Disputed'
  | 'Cancelled';

export interface EscrowItem {
  amount: bigint;
  released: boolean;
  description: string | null;
}

export interface Milestone {
  description: string;
  amount: bigint;
  completed: boolean;
  completed_at: number | null;
}

export interface TimeLock {
  release_ledger: number;
  enabled: boolean;
}

export interface BuyerContribution {
  buyer: string;
  amount: bigint;
  funded: boolean;
}

export interface GroupBuy {
  buyers: BuyerContribution[];
  target_amount: bigint;
  funded_amount: bigint;
  funding_deadline: number;
}

export interface RefundReason {
  ProductNotReceived: null;
}
export type RefundReasonType =
  | 'ProductNotReceived'
  | 'ProductDefective'
  | 'WrongProduct'
  | 'ChangedMind'
  | 'Other';

export type RefundStatus = 'Pending' | 'Approved' | 'Rejected';

export interface RefundRequest {
  request_id: number;
  escrow_id: number;
  requester: string;
  amount: bigint;
  reason: RefundReasonType;
  status: RefundStatus;
  created_at: number;
  evidence_hash: string | null;
  counter_evidence_hash: string | null;
}

export interface RefundHistoryEntry {
  refund_id: number;
  escrow_id: number;
  amount: bigint;
  refunded_at: number;
}

export interface BulkEscrowRequest {
  seller: string;
  amount: bigint;
  metadata: string | null;
  arbiter: string | null;
  items: EscrowItem[] | null;
}

export interface StorageRentEstimate {
  escrow_id: number;
  entry_count: number;
  estimated_bytes: number;
  max_ttl: number;
}

export interface ContractResourceProfile {
  max_items_per_escrow: number;
  max_metadata_size: number;
  unfunded_expiry_ledgers: number;
  evidence_window_ledgers: number;
  appeal_window_ledgers: number;
  max_ttl: number;
}

export interface ArbiterStake {
  arbiter: string;
  token: string;
  amount: bigint;
  staked_at: number;
  slashed: boolean;
  slash_amount: bigint;
}

export interface EvidenceWindow {
  escrow_id: number;
  opened_at: number;
  expires_at: number;
  buyer_submitted: boolean;
  seller_submitted: boolean;
  buyer_evidence_hash: string | null;
  seller_evidence_hash: string | null;
  expired: boolean;
}

export interface AppealRecord {
  escrow_id: number;
  appellant: string;
  filed_at: number;
  resolved: boolean;
  outcome: number | null;
  ruling_ledger: number;
}

export interface ArbiterReputation {
  arbiter: string;
  total_disputes: number;
  resolved_disputes: number;
  appealed_rulings: number;
  overturned_rulings: number;
  slash_count: number;
  last_active: number;
}

export type MetadataVisibility = 'Private' | 'Public';

export interface Escrow {
  buyer: string;
  seller: string;
  token: string;
  amount: bigint;
  status: EscrowStatus;
  metadata: string | null;
  arbiter: string | null;
  cancellation_proposer: string | null;
  items: EscrowItem[];
  created_at: number;
  tracking_id: string | null;
  milestones: Milestone[];
  time_lock: TimeLock[];
  group_buy: GroupBuy[];
}

export interface MediationPhase {
  escrow_id: number;
  opened_at: number;
  expires_at: number;
  buyer_proposal: bigint | null;
  seller_proposal: bigint | null;
  concluded: boolean;
}

export type DataKey =
  | { Escrow: number }
  | 'EscrowCounter'
  | 'FeeCollector'
  | 'FeeBps'
  | 'MinFee'
  | 'MaxFee'
  | 'NativeAsset'
  | 'NativeFeeBps'
  | 'ReentrancyLock'
  | 'Admin'
  | 'ProposedAdmin'
  | 'Paused'
  | { RefundRequest: number }
  | 'RefundCount'
  | { EscrowRefunds: number }
  | { RefundHistory: number }
  | 'GlobalRefundHistory'
  | 'InitialValue'
  | { EscrowHash: string }
  | 'TotalFundedAmount'
  | 'TotalRefundedAmount'
  | 'TotalDisputedCount'
  | 'TotalFeesCollected'
  | 'EscrowIds'
  | 'TotalReleasedAmount'
  | { PendingFee: [string, string] }
  | { FeeWhitelist: string }
  | 'Oracle'
  | { MilestoneEscrow: number }
  | { TimeLockEscrow: number }
  | { GroupBuyEscrow: number }
  | { ArbiterStake: number }
  | 'MinArbiterStake'
  | { EvidenceWindow: number }
  | { Appeal: number }
  | { ArbiterReputation: string }
  | { ClaimableAt: number }
  | { MetadataVisibility: number }
  | 'FeatureDisputesEnabled'
  | 'FeaturePartialReleasesEnabled'
  | { MediationPhase: number }
  | { TokenPaused: string }
  | 'SchemaVersion'
  | { EscrowSchemaVersion: number };

export interface EscrowCreatedEvent {
  escrow_id: number;
  buyer: string;
  seller: string;
  token: string;
  amount: bigint;
  status: EscrowStatus;
  arbiter: string | null;
  tracking_id: string | null;
}

export interface FundsReleasedEvent {
  escrow_id: number;
  amount: bigint;
  fee: bigint;
}

export interface DeliveryVerifiedEvent {
  escrow_id: number;
  tracking_id: string;
}

export interface FeeCollectedEvent {
  escrow_id: number;
  fee_collector: string;
  fee: bigint;
}

export interface FeeCollectorRotatedEvent {
  old_collector: string;
  new_collector: string;
  actor: string;
}

export interface FeesWithdrawnEvent {
  collector: string;
  token: string;
  amount: bigint;
}

export interface StatusChangeEvent {
  escrow_id: number;
  from_status: EscrowStatus;
  to_status: EscrowStatus;
  actor: string;
}

export interface CancellationProposedEvent {
  escrow_id: number;
  actor: string;
}

export interface FeeChangedEvent {
  old_fee_bps: number;
  new_fee_bps: number;
  actor: string;
}

export interface FeeCapsChangedEvent {
  old_min_fee: bigint;
  new_min_fee: bigint;
  old_max_fee: bigint;
  new_max_fee: bigint;
  actor: string;
}

export interface AdminTransferredEvent {
  old_admin: string;
  new_admin: string;
}

export interface RefundRequestedEvent {
  request_id: number;
  escrow_id: number;
  requester: string;
  evidence_hash: string | null;
}

export interface CounterEvidenceSubmittedEvent {
  request_id: number;
  escrow_id: number;
  responder: string;
  counter_evidence_hash: string | null;
}

export interface BulkEscrowCreatedEvent {
  buyer: string;
  token: string;
  escrow_ids: number[];
}

export interface FeeExemptionEvent {
  address: string;
  exempted: boolean;
  actor: string;
}

export interface MilestoneCompletedEvent {
  escrow_id: number;
  milestone_index: number;
  amount: bigint;
}

export interface TimeLockReleasedEvent {
  escrow_id: number;
  amount: bigint;
}

export interface GroupBuyFundedEvent {
  escrow_id: number;
  buyer: string;
  amount: bigint;
}

export interface GroupBuyCompletedEvent {
  escrow_id: number;
  total_amount: bigint;
}

export interface BatchFeesCollectedEvent {
  collector: string;
  token: string;
  total_amount: bigint;
  escrow_count: number;
}

export interface EscrowExpiredEvent {
  escrow_id: number;
  buyer: string;
  seller: string;
}

export interface ArbiterStakedEvent {
  escrow_id: number;
  arbiter: string;
  amount: bigint;
}

export interface ArbiterSlashedEvent {
  escrow_id: number;
  arbiter: string;
  slash_amount: bigint;
}

export interface EvidenceSubmittedEvent {
  escrow_id: number;
  party: string;
  evidence_hash: string;
}

export interface EvidenceWindowExpiredEvent {
  escrow_id: number;
  default_refund: boolean;
}

export interface AppealFiledEvent {
  escrow_id: number;
  appellant: string;
}

export interface AppealResolvedEvent {
  escrow_id: number;
  admin: string;
  outcome: number;
  overturned: boolean;
}

export interface MediationOpenedEvent {
  escrow_id: number;
  expires_at: number;
}

export interface MediationProposedEvent {
  escrow_id: number;
  proposer: string;
  amount: bigint;
}

export interface MediationSettledEvent {
  escrow_id: number;
  seller_amount: bigint;
  buyer_refund: bigint;
}

export interface TokenCircuitBreakerEvent {
  token: string;
  paused: boolean;
  actor: string;
}

export type ContractEvent =
  | { type: 'EscrowCreated'; data: EscrowCreatedEvent }
  | { type: 'FundsReleased'; data: FundsReleasedEvent }
  | { type: 'DeliveryVerified'; data: DeliveryVerifiedEvent }
  | { type: 'FeeCollected'; data: FeeCollectedEvent }
  | { type: 'FeeCollectorRotated'; data: FeeCollectorRotatedEvent }
  | { type: 'FeesWithdrawn'; data: FeesWithdrawnEvent }
  | { type: 'StatusChange'; data: StatusChangeEvent }
  | { type: 'CancellationProposed'; data: CancellationProposedEvent }
  | { type: 'FeeChanged'; data: FeeChangedEvent }
  | { type: 'FeeCapsChanged'; data: FeeCapsChangedEvent }
  | { type: 'AdminTransferred'; data: AdminTransferredEvent }
  | { type: 'RefundRequested'; data: RefundRequestedEvent }
  | { type: 'CounterEvidenceSubmitted'; data: CounterEvidenceSubmittedEvent }
  | { type: 'BulkEscrowCreated'; data: BulkEscrowCreatedEvent }
  | { type: 'FeeExemption'; data: FeeExemptionEvent }
  | { type: 'MilestoneCompleted'; data: MilestoneCompletedEvent }
  | { type: 'TimeLockReleased'; data: TimeLockReleasedEvent }
  | { type: 'GroupBuyFunded'; data: GroupBuyFundedEvent }
  | { type: 'GroupBuyCompleted'; data: GroupBuyCompletedEvent }
  | { type: 'BatchFeesCollected'; data: BatchFeesCollectedEvent }
  | { type: 'EscrowExpired'; data: EscrowExpiredEvent }
  | { type: 'ArbiterStaked'; data: ArbiterStakedEvent }
  | { type: 'ArbiterSlashed'; data: ArbiterSlashedEvent }
  | { type: 'EvidenceSubmitted'; data: EvidenceSubmittedEvent }
  | { type: 'EvidenceWindowExpired'; data: EvidenceWindowExpiredEvent }
  | { type: 'AppealFiled'; data: AppealFiledEvent }
  | { type: 'AppealResolved'; data: AppealResolvedEvent }
  | { type: 'MediationOpened'; data: MediationOpenedEvent }
  | { type: 'MediationProposed'; data: MediationProposedEvent }
  | { type: 'MediationSettled'; data: MediationSettledEvent }
  | { type: 'TokenCircuitBreaker'; data: TokenCircuitBreakerEvent };
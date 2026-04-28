/**
 * MarketX Contract — Error Code Mapping
 *
 * Maps every `ContractError` discriminant to a user-friendly message and an
 * optional recovery hint. Import this file into any frontend or SDK that
 * interacts with the MarketX escrow contract.
 *
 * Usage:
 *   import { getContractError, MARKETX_ERRORS } from './error-codes';
 *
 *   const err = getContractError(10);
 *   // { code: 'EscrowNotFound', message: 'Escrow not found.', recovery: 'Check that the escrow ID is correct.' }
 */

export interface ContractErrorInfo {
  /** Rust variant name for debugging / logging. */
  code: string;
  /** Human-readable sentence suitable for display in a UI. */
  message: string;
  /** Optional guidance the UI can show to help the user recover. Empty string when there is nothing actionable. */
  recovery: string;
}

export const MARKETX_ERRORS: Readonly<Record<number, ContractErrorInfo>> = {
  // ── Access Control ────────────────────────────────────────────────────────
  1: {
    code: 'NotAdmin',
    message: 'Only the contract admin can perform this action.',
    recovery: '',
  },
  2: {
    code: 'Unauthorized',
    message: 'You are not authorized to perform this operation.',
    recovery: 'Ensure you are a party to the escrow (buyer, seller, or arbiter).',
  },
  3: {
    code: 'NotProposedAdmin',
    message: 'This address is not the proposed new admin.',
    recovery: '',
  },
  4: {
    code: 'NotOracle',
    message: 'Only the registered oracle may call this function.',
    recovery: '',
  },

  // ── Escrow State ──────────────────────────────────────────────────────────
  10: {
    code: 'EscrowNotFound',
    message: 'Escrow not found.',
    recovery: 'Check that the escrow ID is correct.',
  },
  11: {
    code: 'InvalidEscrowState',
    message: 'The escrow is in the wrong state for this action.',
    recovery: 'Check the escrow status before calling.',
  },
  13: {
    code: 'InvalidEscrowAmount',
    message: 'Invalid escrow amount — must be greater than zero.',
    recovery: '',
  },

  // ── Circuit Breaker ───────────────────────────────────────────────────────
  31: {
    code: 'ContractPaused',
    message: 'The contract is currently paused. Please try again later.',
    recovery: 'Wait for the admin to unpause the contract.',
  },

  // ── Overflow Protection ───────────────────────────────────────────────────
  40: {
    code: 'EscrowIdOverflow',
    message: 'The maximum number of escrows has been reached.',
    recovery: '',
  },

  // ── Fee Configuration ─────────────────────────────────────────────────────
  50: {
    code: 'InvalidFeeConfig',
    message: 'The fee configuration is invalid.',
    recovery: 'Ensure fee values are within allowed bounds.',
  },

  // ── Input Validation ──────────────────────────────────────────────────────
  60: {
    code: 'MetadataTooLarge',
    message: 'A metadata or description field exceeds the maximum allowed size.',
    recovery:
      'Reduce the size of the metadata, description, tracking ID, or evidence hash. Limits: metadata 1 024 B, descriptions 256 B, tracking ID 128 B, evidence hash 128 B.',
  },

  // ── Duplicate Prevention ──────────────────────────────────────────────────
  70: {
    code: 'DuplicateEscrow',
    message: 'An escrow with the same buyer, seller, and metadata already exists.',
    recovery: 'Change the metadata to create a distinct escrow.',
  },

  // ── Items ─────────────────────────────────────────────────────────────────
  80: {
    code: 'ItemNotFound',
    message: 'Item index not found in this escrow.',
    recovery: 'Check the item index.',
  },
  81: {
    code: 'ItemAlreadyReleased',
    message: 'This item has already been released.',
    recovery: '',
  },
  82: {
    code: 'TooManyItems',
    message: 'Too many items per escrow (maximum: 50).',
    recovery: 'Split the escrow into multiple escrows.',
  },
  83: {
    code: 'ItemAmountInvalid',
    message: 'Item amounts do not sum to the total escrow amount.',
    recovery: 'Ensure all item amounts add up to the escrow total.',
  },

  // ── Expiry and Funding ────────────────────────────────────────────────────
  90: {
    code: 'EscrowNotExpired',
    message: 'The escrow has not yet expired.',
    recovery: 'Wait for the unfunded expiry window (~7 days) to pass.',
  },
  91: {
    code: 'EscrowAlreadyFunded',
    message: 'The escrow has already been funded.',
    recovery: '',
  },

  // ── Milestones ────────────────────────────────────────────────────────────
  100: {
    code: 'MilestoneNotFound',
    message: 'Milestone index not found.',
    recovery: 'Check the milestone index.',
  },
  101: {
    code: 'MilestoneAlreadyCompleted',
    message: 'This milestone has already been completed.',
    recovery: '',
  },

  // ── Time-Lock ─────────────────────────────────────────────────────────────
  110: {
    code: 'TimeLockNotReached',
    message: 'The time-lock release ledger has not been reached yet.',
    recovery: 'Wait until the configured release ledger.',
  },
  111: {
    code: 'TimeLockNotEnabled',
    message: 'No time-lock is configured for this escrow.',
    recovery: '',
  },

  // ── Group Buy ─────────────────────────────────────────────────────────────
  120: {
    code: 'GroupBuyNotFunded',
    message: 'The group buy target amount has not been reached yet.',
    recovery: '',
  },
  121: {
    code: 'GroupBuyAlreadyFunded',
    message: 'The group buy has already been fully funded.',
    recovery: '',
  },
  122: {
    code: 'GroupBuyDeadlinePassed',
    message: 'The group buy funding deadline has passed.',
    recovery: '',
  },
  123: {
    code: 'InvalidGroupBuyAmount',
    message: 'Invalid group buy contribution amount.',
    recovery: '',
  },

  // ── Dispute Resolution — Arbiter Staking ──────────────────────────────────
  130: {
    code: 'ArbiterStakeInsufficient',
    message: 'The arbiter stake is below the required minimum.',
    recovery: 'Increase the stake amount to meet the minimum requirement.',
  },
  131: {
    code: 'ArbiterAlreadyStaked',
    message: 'The arbiter already has an active stake on this escrow.',
    recovery: '',
  },
  132: {
    code: 'ArbiterMismatch',
    message: 'The caller is not the registered arbiter for this escrow.',
    recovery: '',
  },

  // ── Dispute Resolution — Evidence Window ──────────────────────────────────
  140: {
    code: 'EvidenceWindowExpired',
    message: 'The evidence submission window has closed. No further submissions are accepted.',
    recovery: '',
  },
  141: {
    code: 'EvidenceWindowNotExpired',
    message: 'The evidence window has not yet expired.',
    recovery: 'Wait for the evidence window to expire before forcing closure.',
  },
  142: {
    code: 'NoEvidenceWindow',
    message: 'No evidence window is open for this escrow.',
    recovery: '',
  },

  // ── Dispute Resolution — Appeals ──────────────────────────────────────────
  150: {
    code: 'AppealAlreadyFiled',
    message: 'An appeal has already been filed for this escrow.',
    recovery: '',
  },
  151: {
    code: 'AppealNotFound',
    message: 'No appeal record exists for this escrow.',
    recovery: '',
  },
  152: {
    code: 'AppealWindowClosed',
    message: 'The appeal window has closed.',
    recovery: '',
  },
  153: {
    code: 'AppealAlreadyResolved',
    message: 'The appeal has already been resolved.',
    recovery: '',
  },
} as const;

/**
 * Look up a contract error by its numeric code.
 * Returns `undefined` when the code is not recognised.
 */
export function getContractError(code: number): ContractErrorInfo | undefined {
  return MARKETX_ERRORS[code];
}

/**
 * Return a display-ready error message for a given code.
 * Falls back to a generic message when the code is unknown.
 */
export function getErrorMessage(code: number): string {
  return MARKETX_ERRORS[code]?.message ?? `Unknown contract error (code ${code}).`;
}

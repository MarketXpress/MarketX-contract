const federationQuerySchema = z.object({
  q: z.string().min(1, "q is required"),
  type: z.enum(["name", "id", "txid", "forward"] as const, {
    error: () => ({ message: "type must be one of: name, id, txid, forward" }),
  }),
});if (!parsed.success) {
  return res.status(400).json({
    detail: parsed.error?.issues?.[0]?.message ?? "Invalid input",
  });
}pub use types::{
    AdminTransferredEvent,
    BulkEscrowCreatedEvent,
    BulkEscrowRequest,
    CancellationProposedEvent,
    CounterEvidenceSubmittedEvent,
    DataKey,
    Escrow,
    EscrowCreatedEvent,
    EscrowExpiredEvent,
    EscrowItem,
    EscrowStatus,
    FeeCapsChangedEvent,
    FeeChangedEvent,
    FeeCollectedEvent,
    FeeExemptionEvent,
    FeesWithdrawnEvent,
    FundsReleasedEvent,
    RefundHistoryEntry,
    RefundReason,
    RefundRequest,
    RefundRequestedEvent,
    RefundStatus,
    StatusChangeEvent,
    MAX_ITEMS_PER_ESCROW,
    MAX_METADATA_SIZE,
    UNFUNDED_EXPIRY_LEDGERS,
};
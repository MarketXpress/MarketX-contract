#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Escrow, EscrowStatus};

    #[test]
    fn test_partial_release() {
        let mut escrow = Escrow {
            id: "escrow1".to_string(),
            buyer: "buyer1".to_string(),
            seller: "seller1".to_string(),
            amount: 100,
            released_amount: 0,
            refunded_amount: 0,
            status: EscrowStatus::Locked,
        };

        let result = release_partial(&mut escrow, 60);
        assert!(result.is_ok());
        assert_eq!(escrow.released_amount, 60);
        assert_eq!(escrow.refunded_amount, 40);
        assert_eq!(escrow.status, EscrowStatus::PartiallyReleased);
    }
}

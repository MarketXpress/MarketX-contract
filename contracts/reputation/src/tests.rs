#![cfg(test)]

use super::*;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Env, String};

#[test]
fn test_reputation_flow() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ReputationContract, ());
    let client = ReputationContractClient::new(&env, &contract_id);

    let reviewer1 = Address::generate(&env);
    let reviewer2 = Address::generate(&env);
    let subject = Address::generate(&env);

    // 1. Submit first review (5 stars)
    let rep1 = client.submit_review(
        &reviewer1,
        &subject,
        &5,
        &String::from_str(&env, "Great service!"),
    );

    assert_eq!(rep1.total_reviews, 1);
    assert_eq!(rep1.total_score, 5);
    assert_eq!(rep1.average_rating, 500); // 5.00

    // 2. Submit second review (4 stars)
    let rep2 = client.submit_review(
        &reviewer2,
        &subject,
        &4,
        &String::from_str(&env, "Good enough"),
    );

    assert_eq!(rep2.total_reviews, 2);
    assert_eq!(rep2.total_score, 9);
    assert_eq!(rep2.average_rating, 450); // 4.50

    // 3. Verify reviews stored
    let reviews = client.get_reviews(&subject);
    assert_eq!(reviews.len(), 2);

    let r1 = reviews.get(0).unwrap();
    assert_eq!(r1.reviewer, reviewer1);
    assert_eq!(r1.rating, 5);

    let r2 = reviews.get(1).unwrap();
    assert_eq!(r2.reviewer, reviewer2);
    assert_eq!(r2.rating, 4);
}

#[test]
#[should_panic(expected = "Self-review is not allowed")]
fn test_self_review_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ReputationContract, ());
    let client = ReputationContractClient::new(&env, &contract_id);

    let user = Address::generate(&env);

    client.submit_review(&user, &user, &5, &String::from_str(&env, "I am the best"));
}

#[test]
#[should_panic(expected = "Rating must be between 1 and 5")]
fn test_invalid_rating_fails() {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(ReputationContract, ());
    let client = ReputationContractClient::new(&env, &contract_id);

    let reviewer = Address::generate(&env);
    let subject = Address::generate(&env);

    client.submit_review(&reviewer, &subject, &6, &String::from_str(&env, "Too good"));
}

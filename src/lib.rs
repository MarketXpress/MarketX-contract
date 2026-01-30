#![no_std]

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, BytesN, Env, Symbol, Vec as SVec, Map as SMap, IntoVal};


#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub enum DataKey {
    Admin,
    FeeBps,
    FeeCollector,
    EmergencyAdmins,
    EmergencyThreshold,
    Escrow(u128),
    PendingRelease(u128),
    ApprovalsRelease(u128),
    ApprovalsRefund(u128),
    ApprovalsArbiter(u128),
    ApprovalsEmergency(u128),
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct EscrowInit {
    pub token: Address,
    pub payers: SVec<Address>,
    pub payees: SVec<Address>,
    pub release_signers: SVec<Address>,
    pub release_threshold: u32,
    pub refund_signers: SVec<Address>,
    pub refund_threshold: u32,
    pub arbiters: SVec<Address>,
    pub arbiter_threshold: u32,
    pub auto_release_ts: Option<u64>,
    pub expiry_ts: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct Escrow {
    pub token: Address,
    pub payers: SVec<Address>,
    pub payees: SVec<Address>,
    pub release_signers: SVec<Address>,
    pub release_threshold: u32,
    pub refund_signers: SVec<Address>,
    pub refund_threshold: u32,
    pub arbiters: SVec<Address>,
    pub arbiter_threshold: u32,
    pub auto_release_ts: Option<u64>,
    pub expiry_ts: u64,
    pub disputed: bool,
    pub balance: i128,
    pub deposits: SVec<(Address, i128)>,
    pub closed: bool,
    pub nonce: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
#[contracttype]
pub struct ReleaseProposal {
    pub nonce: u64,
    pub dists: SVec<(Address, i128)>,
}

fn read_u32(env: &Env, key: &DataKey) -> u32 { env.storage().persistent().get::<_, u32>(key).unwrap() }
fn write_u32(env: &Env, key: &DataKey, v: u32) { env.storage().persistent().set(key, &v) }
fn read_addr(env: &Env, key: &DataKey) -> Address { env.storage().persistent().get::<_, Address>(key).unwrap() }
fn write_addr(env: &Env, key: &DataKey, v: &Address) { env.storage().persistent().set(key, v) }
fn read_vec_addr(env: &Env, key: &DataKey) -> SVec<Address> { env.storage().persistent().get::<_, SVec<Address>>(key).unwrap_or_else(|| SVec::new(env)) }
fn write_vec_addr(env: &Env, key: &DataKey, v: &SVec<Address>) { env.storage().persistent().set(key, v) }

fn read_escrow(env: &Env, id: u128) -> Escrow { env.storage().persistent().get::<_, Escrow>(&DataKey::Escrow(id)).unwrap() }
fn write_escrow(env: &Env, id: u128, e: &Escrow) { env.storage().persistent().set(&DataKey::Escrow(id), e) }

fn token_client(env: &Env, addr: &Address) -> soroban_sdk::token::Client { soroban_sdk::token::Client::new(env, addr) }

fn is_member(list: &SVec<Address>, who: &Address) -> bool { list.iter().any(|a| a == who.clone()) }

fn push_unique(list: &mut SVec<Address>, who: &Address) {
    if !is_member(list, who) { list.push_back(who.clone()); }
}

fn sum_amounts(dists: &SVec<(Address, i128)>) -> i128 {
    let mut s: i128 = 0;
    for (_, a) in dists.iter() { s += a; }
    s
}

fn ensure_payees_valid(payees: &SVec<Address>, dists: &SVec<(Address, i128)>) -> bool {
    for (p, _) in dists.iter() { if !is_member(payees, &p) { return false; } }
    true
}

fn now(env: &Env) -> u64 { env.ledger().timestamp() }

#[contract]
pub struct MarketXEscrow;

#[contractimpl]
impl MarketXEscrow {
    // Admin setup
    pub fn init(env: Env, admin: Address, fee_bps: u32, fee_collector: Address, emergency_admins: SVec<Address>, emergency_threshold: u32) {
        if env.storage().persistent().has(&DataKey::Admin) { panic!("already-initialized"); }
        if fee_bps > 10_000 { panic!("fee-bps-range"); }
        if emergency_threshold == 0 || emergency_threshold as usize > emergency_admins.len() { panic!("bad-emergency-threshold"); }
        write_addr(&env, &DataKey::Admin, &admin);
        write_u32(&env, &DataKey::FeeBps, fee_bps);
        write_addr(&env, &DataKey::FeeCollector, &fee_collector);
        write_vec_addr(&env, &DataKey::EmergencyAdmins, &emergency_admins);
        write_u32(&env, &DataKey::EmergencyThreshold, emergency_threshold);
    }

    pub fn set_fees(env: Env, admin: Address, fee_bps: u32, fee_collector: Address) {
        let a = read_addr(&env, &DataKey::Admin);
        if admin != a { panic!("not-admin"); }
        admin.require_auth();
        if fee_bps > 10_000 { panic!("fee-bps-range"); }
        write_u32(&env, &DataKey::FeeBps, fee_bps);
        write_addr(&env, &DataKey::FeeCollector, &fee_collector);
    }

    pub fn set_emergency(env: Env, admin: Address, admins: SVec<Address>, threshold: u32) {
        let a = read_addr(&env, &DataKey::Admin);
        if admin != a { panic!("not-admin"); }
        admin.require_auth();
        if threshold == 0 || threshold as usize > admins.len() { panic!("bad-threshold"); }
        write_vec_addr(&env, &DataKey::EmergencyAdmins, &admins);
        write_u32(&env, &DataKey::EmergencyThreshold, threshold);
    }

    // Escrow lifecycle
    pub fn create_escrow(env: Env, id: u128, params: EscrowInit) {
        if env.storage().persistent().has(&DataKey::Escrow(id)) { panic!("exists"); }
        if params.release_threshold == 0 || params.release_threshold as usize > params.release_signers.len() { panic!("bad-release-thresh"); }
        if params.refund_threshold == 0 || params.refund_threshold as usize > params.refund_signers.len() { panic!("bad-refund-thresh"); }
        if params.arbiter_threshold == 0 || params.arbiter_threshold as usize > params.arbiters.len() { panic!("bad-arb-thresh"); }
        if params.payers.is_empty() || params.payees.is_empty() { panic!("empty-parties"); }
        let e = Escrow {
            token: params.token,
            payers: params.payers,
            payees: params.payees,
            release_signers: params.release_signers,
            release_threshold: params.release_threshold,
            refund_signers: params.refund_signers,
            refund_threshold: params.refund_threshold,
            arbiters: params.arbiters,
            arbiter_threshold: params.arbiter_threshold,
            auto_release_ts: params.auto_release_ts,
            expiry_ts: params.expiry_ts,
            disputed: false,
            balance: 0,
            deposits: SVec::new(&env),
            closed: false,
            nonce: 0,
        };
        write_escrow(&env, id, &e);
    }

    pub fn deposit(env: Env, id: u128, from: Address, amount: i128) {
        if amount <= 0 { panic!("bad-amount"); }
        let mut e = read_escrow(&env, id);
        if e.closed { panic!("closed"); }
        if !is_member(&e.payers, &from) { panic!("not-payer"); }
        from.require_auth();
        let client = token_client(&env, &e.token);
        client.transfer(&from, &env.current_contract_address(), &amount);
        e.balance += amount;
        // update deposits
        let mut found = false;
        let mut out = SVec::new(&env);
        for (p, a) in e.deposits.iter() {
            if p == from { out.push_back((p, a + amount)); found = true; } else { out.push_back((p, a)); }
        }
        if !found { out.push_back((from, amount)); }
        e.deposits = out;
        write_escrow(&env, id, &e);
    }

    pub fn open_dispute(env: Env, id: u128, actor: Address) {
        let mut e = read_escrow(&env, id);
        if e.closed { panic!("closed"); }
        if !(is_member(&e.payers, &actor) || is_member(&e.payees, &actor)) { panic!("no-right"); }
        actor.require_auth();
        e.disputed = true;
        write_escrow(&env, id, &e);
    }

    pub fn propose_release(env: Env, id: u128, signer: Address, dists: SVec<(Address, i128)>) {
        let mut e = read_escrow(&env, id);
        if e.closed { panic!("closed"); }
        if e.disputed { panic!("disputed"); }
        if !is_member(&e.release_signers, &signer) { panic!("not-release-signer"); }
        signer.require_auth();
        if !ensure_payees_valid(&e.payees, &dists) { panic!("bad-payee"); }
        let total = sum_amounts(&dists);
        if total <= 0 || total > e.balance { panic!("bad-total"); }
        e.nonce += 1;
        let prop = ReleaseProposal { nonce: e.nonce, dists: dists.clone() };
        env.storage().persistent().set(&DataKey::PendingRelease(id), &prop);
        let mut approvers = SVec::new(&env);
        approvers.push_back(signer);
        env.storage().persistent().set(&DataKey::ApprovalsRelease(id), &approvers);
        write_escrow(&env, id, &e);
    }

    pub fn approve_release(env: Env, id: u128, signer: Address) {
        let e = read_escrow(&env, id);
        if e.closed { panic!("closed"); }
        if e.disputed { panic!("disputed"); }
        if !is_member(&e.release_signers, &signer) { panic!("not-release-signer"); }
        signer.require_auth();
        let mut approvers = env.storage().persistent().get::<_, SVec<Address>>(&DataKey::ApprovalsRelease(id)).unwrap_or_else(|| SVec::new(&env));
        push_unique(&mut approvers, &signer);
        env.storage().persistent().set(&DataKey::ApprovalsRelease(id), &approvers);
        if approvers.len() as u32 >= e.release_threshold {
            Self::execute_pending_release(env, id);
        }
    }

    fn execute_pending_release(env: Env, id: u128) {
        let mut e = read_escrow(&env, id);
        let prop: ReleaseProposal = env.storage().persistent().get(&DataKey::PendingRelease(id)).unwrap();
        let dists = prop.dists;
        let total = sum_amounts(&dists);
        if total <= 0 || total > e.balance { panic!("bad-total"); }
        let fee_bps = read_u32(&env, &DataKey::FeeBps) as i128;
        let fee_collector = read_addr(&env, &DataKey::FeeCollector);
        let client = token_client(&env, &e.token);
        // Transfer per distribution after fee
        let mut fee_total: i128 = 0;
        for (to, amt) in dists.iter() {
            let fee = amt * fee_bps / 10_000;
            let net = amt - fee;
            if net < 0 { panic!("fee-too-high"); }
            if fee > 0 { fee_total += fee; }
            client.transfer(&env.current_contract_address(), &to, &net);
        }
        if fee_total > 0 { client.transfer(&env.current_contract_address(), &fee_collector, &fee_total); }
        e.balance -= total;
        if e.balance == 0 { e.closed = true; }
        // clear pending
        env.storage().persistent().remove(&DataKey::PendingRelease(id));
        env.storage().persistent().remove(&DataKey::ApprovalsRelease(id));
        write_escrow(&env, id, &e);
    }

    pub fn propose_refund(env: Env, id: u128, signer: Address, dists: SVec<(Address, i128)>) {
        // dists target payers
        let e = read_escrow(&env, id);
        if e.closed { panic!("closed"); }
        if e.disputed { panic!("disputed"); }
        if !is_member(&e.refund_signers, &signer) { panic!("not-refund-signer"); }
        signer.require_auth();
        // validate recipients are payers
        for (p, _) in dists.iter() { if !is_member(&e.payers, &p) { panic!("bad-payer"); } }
        let total = sum_amounts(&dists);
        if total <= 0 || total > e.balance { panic!("bad-total"); }
        env.storage().persistent().set(&DataKey::PendingRelease(id), &ReleaseProposal { nonce: e.nonce + 1, dists: dists.clone() });
        let mut approvers = SVec::new(&env);
        approvers.push_back(signer);
        env.storage().persistent().set(&DataKey::ApprovalsRefund(id), &approvers);
    }

    pub fn approve_refund(env: Env, id: u128, signer: Address) {
        let mut e = read_escrow(&env, id);
        if e.closed { panic!("closed"); }
        if e.disputed { panic!("disputed"); }
        if !is_member(&e.refund_signers, &signer) { panic!("not-refund-signer"); }
        signer.require_auth();
        let mut approvers = env.storage().persistent().get::<_, SVec<Address>>(&DataKey::ApprovalsRefund(id)).unwrap_or_else(|| SVec::new(&env));
        push_unique(&mut approvers, &signer);
        env.storage().persistent().set(&DataKey::ApprovalsRefund(id), &approvers);
        if approvers.len() as u32 >= e.refund_threshold {
            // execute pending refund
            let prop: ReleaseProposal = env.storage().persistent().get(&DataKey::PendingRelease(id)).unwrap();
            let dists = prop.dists;
            let total = sum_amounts(&dists);
            if total <= 0 || total > e.balance { panic!("bad-total"); }
            let client = token_client(&env, &e.token);
            for (to, amt) in dists.iter() {
                client.transfer(&env.current_contract_address(), &to, &amt);
            }
            e.balance -= total;
            if e.balance == 0 { e.closed = true; }
            env.storage().persistent().remove(&DataKey::PendingRelease(id));
            env.storage().persistent().remove(&DataKey::ApprovalsRefund(id));
            write_escrow(&env, id, &e);
        }
    }

    pub fn refund_timeout(env: Env, id: u128) {
        let mut e = read_escrow(&env, id);
        if e.closed { panic!("closed"); }
        if e.disputed { panic!("disputed"); }
        if now(&env) < e.expiry_ts { panic!("not-expired"); }
        let client = token_client(&env, &e.token);
        let mut remaining = e.balance;
        for (p, a) in e.deposits.iter() {
            if remaining <= 0 { break; }
            let amt = if a <= remaining { a } else { remaining };
            if amt > 0 { client.transfer(&env.current_contract_address(), &p, &amt); }
            remaining -= amt;
        }
        e.balance = remaining;
        if e.balance == 0 { e.closed = true; }
        write_escrow(&env, id, &e);
    }

    pub fn auto_release(env: Env, id: u128) {
        let e = read_escrow(&env, id);
        if e.closed { panic!("closed"); }
        if e.disputed { panic!("disputed"); }
        match e.auto_release_ts { Some(t) => if now(&env) < t { panic!("too-early"); }, None => panic!("no-auto-release") }
        // equal split among payees
        let n = e.payees.len() as i128;
        if n <= 0 { panic!("no-payees"); }
        let mut dists: SVec<(Address, i128)> = SVec::new(&env);
        let base = e.balance / n;
        let mut rem = e.balance - base * n;
        for i in 0..e.payees.len() {
            let mut amt = base;
            if rem > 0 { amt += 1; rem -= 1; }
            dists.push_back((e.payees.get_unchecked(i).unwrap(), amt));
        }
        drop(e);
        // reuse propose->execute path: set pending and approvals as threshold satisfied
        let mut e2 = read_escrow(&env, id);
        e2.nonce += 1;
        let prop = ReleaseProposal { nonce: e2.nonce, dists: dists.clone() };
        env.storage().persistent().set(&DataKey::PendingRelease(id), &prop);
        let mut approvers = SVec::new(&env);
        // fake approvals: set len == threshold
        for i in 0..e2.release_threshold { approvers.push_back(env.current_contract_address()); }
        env.storage().persistent().set(&DataKey::ApprovalsRelease(id), &approvers);
        write_escrow(&env, id, &e2);
        Self::execute_pending_release(env, id);
    }

    pub fn arbiter_release(env: Env, id: u128, signer: Address, dists: SVec<(Address, i128)>) {
        let e = read_escrow(&env, id);
        if e.closed { panic!("closed"); }
        if !e.disputed { panic!("not-disputed"); }
        if !is_member(&e.arbiters, &signer) { panic!("not-arbiter"); }
        signer.require_auth();
        if !ensure_payees_valid(&e.payees, &dists) { panic!("bad-payee"); }
        let total = sum_amounts(&dists);
        if total <= 0 || total > e.balance { panic!("bad-total"); }
        env.storage().persistent().set(&DataKey::PendingRelease(id), &ReleaseProposal { nonce: e.nonce + 1, dists: dists.clone() });
        let mut approvers = env.storage().persistent().get::<_, SVec<Address>>(&DataKey::ApprovalsArbiter(id)).unwrap_or_else(|| SVec::new(&env));
        push_unique(&mut approvers, &signer);
        env.storage().persistent().set(&DataKey::ApprovalsArbiter(id), &approvers);
        if approvers.len() as u32 >= e.arbiter_threshold {
            // execute like normal release
            Self::execute_pending_release(env, id);
            // clear arbiter approvals and undispute if closed
            env.storage().persistent().remove(&DataKey::ApprovalsArbiter(id));
            let mut e2 = read_escrow(&env, id);
            if e2.balance == 0 { e2.disputed = false; e2.closed = true; write_escrow(&env, id, &e2); }
        }
    }

    pub fn arbiter_refund(env: Env, id: u128, signer: Address, dists: SVec<(Address, i128)>) {
        let mut e = read_escrow(&env, id);
        if e.closed { panic!("closed"); }
        if !e.disputed { panic!("not-disputed"); }
        if !is_member(&e.arbiters, &signer) { panic!("not-arbiter"); }
        signer.require_auth();
        // validate recipients are payers
        for (p, _) in dists.iter() { if !is_member(&e.payers, &p) { panic!("bad-payer"); } }
        let total = sum_amounts(&dists);
        if total <= 0 || total > e.balance { panic!("bad-total"); }
        let mut approvers = env.storage().persistent().get::<_, SVec<Address>>(&DataKey::ApprovalsArbiter(id)).unwrap_or_else(|| SVec::new(&env));
        push_unique(&mut approvers, &signer);
        env.storage().persistent().set(&DataKey::ApprovalsArbiter(id), &approvers);
        if approvers.len() as u32 >= e.arbiter_threshold {
            let client = token_client(&env, &e.token);
            for (to, amt) in dists.iter() { client.transfer(&env.current_contract_address(), &to, &amt); }
            e.balance -= total;
            if e.balance == 0 { e.closed = true; e.disputed = false; }
            env.storage().persistent().remove(&DataKey::ApprovalsArbiter(id));
            write_escrow(&env, id, &e);
        }
    }

    pub fn emergency_release(env: Env, id: u128, signer: Address, dists: SVec<(Address, i128)>) {
        let eadmins = read_vec_addr(&env, &DataKey::EmergencyAdmins);
        let ethresh = read_u32(&env, &DataKey::EmergencyThreshold);
        if !is_member(&eadmins, &signer) { panic!("not-emergency"); }
        signer.require_auth();
        let mut e = read_escrow(&env, id);
        if e.closed { panic!("closed"); }
        let total = sum_amounts(&dists);
        if total <= 0 || total > e.balance { panic!("bad-total"); }
        env.storage().persistent().set(&DataKey::PendingRelease(id), &ReleaseProposal { nonce: e.nonce + 1, dists: dists.clone() });
        let mut approvers = env.storage().persistent().get::<_, SVec<Address>>(&DataKey::ApprovalsEmergency(id)).unwrap_or_else(|| SVec::new(&env));
        push_unique(&mut approvers, &signer);
        env.storage().persistent().set(&DataKey::ApprovalsEmergency(id), &approvers);
        if approvers.len() as u32 >= ethresh {
            Self::execute_pending_release(env, id);
            env.storage().persistent().remove(&DataKey::ApprovalsEmergency(id));
        }
    }

    // Views
    pub fn get_escrow(env: Env, id: u128) -> Escrow { read_escrow(&env, id) }
    pub fn get_fee_params(env: Env) -> (u32, Address) { (read_u32(&env, &DataKey::FeeBps), read_addr(&env, &DataKey::FeeCollector)) }
}

#[cfg(test)]
mod test {
    use super::*;
    use soroban_sdk::{testutils::{Address as _, Ledger}, Address, Env, Vec as SVec, String};

    fn deploy_token(e: &Env, admin: &Address) -> Address {
        use soroban_token_contract::{Token, Client as TokenClient};
        let token_id = e.register_contract(None, Token);
        let client = TokenClient::new(e, &token_id);
        client.initialize(admin, &7, &String::from_str(e, "MarketX Token"), &String::from_str(e, "MXT"));
        token_id
    }

    fn deploy_contract(e: &Env) -> (Address, super::MarketXEscrowClient) {
        let id = e.register_contract(None, MarketXEscrow);
        let client = MarketXEscrowClient::new(e, &id);
        (id, client)
    }

    #[test]
    fn test_full_flow() {
        let e = Env::default();
        e.mock_all_auths();
        e.ledger().set(Ledger { timestamp: 1000, protocol_version: 21, sequence_number: 1, network_passphrase: Default::default(), base_reserve: 0 });

        let admin = Address::generate(&e);
        let fee_collector = Address::generate(&e);
        let emergency1 = Address::generate(&e);
        let emergency2 = Address::generate(&e);
        let (contract_id, client) = deploy_contract(&e);

        let mut eadmins = SVec::new(&e); eadmins.push_back(emergency1.clone()); eadmins.push_back(emergency2.clone());
        client.init(&admin, &250u32, &fee_collector, &eadmins, &2u32);

        let token_admin = Address::generate(&e);
        let token_addr = deploy_token(&e, &token_admin);
        let token_client = soroban_token_contract::Client::new(&e, &token_addr);
        
        let payer1 = Address::generate(&e);
        let payer2 = Address::generate(&e);
        let payee1 = Address::generate(&e);
        let payee2 = Address::generate(&e);
        token_client.mint(&payer1, &1_000);
        token_client.mint(&payer2, &1_000);

        // create escrow
        let mut payers = SVec::new(&e); payers.push_back(payer1.clone()); payers.push_back(payer2.clone());
        let mut payees = SVec::new(&e); payees.push_back(payee1.clone()); payees.push_back(payee2.clone());
        let mut rels = SVec::new(&e); rels.push_back(payer1.clone()); rels.push_back(payer2.clone());
        let mut refs = SVec::new(&e); refs.push_back(payer1.clone()); refs.push_back(payer2.clone());
        let mut arbs = SVec::new(&e); arbs.push_back(Address::generate(&e)); arbs.push_back(Address::generate(&e));

        client.create_escrow(&1u128, &EscrowInit { token: token_addr.clone(), payers: payers.clone(), payees: payees.clone(), release_signers: rels.clone(), release_threshold: 2, refund_signers: refs.clone(), refund_threshold: 1, arbiters: arbs.clone(), arbiter_threshold: 2, auto_release_ts: Some(2000), expiry_ts: 3000 });

        // deposit
        client.deposit(&1u128, &payer1, &600);
        client.deposit(&1u128, &payer2, &400);

        // propose partial release 300 to payee1 and 100 to payee2
        let mut dists = SVec::new(&e); dists.push_back((payee1.clone(), 300)); dists.push_back((payee2.clone(), 100));
        client.propose_release(&1u128, &payer1, &dists);
        client.approve_release(&1u128, &payer2);

        // Verify balances after 400 released with 2.5% fee each
        // net to payee1: 300 - 7 = 293 ; payee2: 100 - 2 = 98 ; fees total 9 sent to collector
        assert_eq!(token_client.balance(&payee1), 293);
        assert_eq!(token_client.balance(&payee2), 98);
        assert_eq!(token_client.balance(&fee_collector), 9);

        // Remaining balance 600
        assert_eq!(client.get_escrow(&1u128).balance, 600);

        // open dispute
        client.open_dispute(&1u128, &payer1);
        // try timeout refund before expiry - should panic if tried. Skip.

        // arbiter resolves by releasing 500 (equal 250/250)
        let mut arb_d = SVec::new(&e); arb_d.push_back((payee1.clone(), 250)); arb_d.push_back((payee2.clone(), 250));
        let arb1 = arbs.get_unchecked(0).unwrap();
        let arb2 = arbs.get_unchecked(1).unwrap();
        client.arbiter_release(&1u128, &arb1, &arb_d);
        client.arbiter_release(&1u128, &arb2, &arb_d);

        // After fee 2.5% each -> 243 and 243, fees 14 -> total fees 23
        assert_eq!(token_client.balance(&payee1), 293 + 243);
        assert_eq!(token_client.balance(&payee2), 98 + 243);
        assert_eq!(token_client.balance(&fee_collector), 9 + 14);

        // Remaining balance 100
        assert_eq!(client.get_escrow(&1u128).balance, 100);

        // Emergency release last 100 to payee1 by emergency admins
        let mut last = SVec::new(&e); last.push_back((payee1.clone(), 100));
        client.emergency_release(&1u128, &emergency1, &last);
        client.emergency_release(&1u128, &emergency2, &last);

        // Fee 2 -> net 98
        assert_eq!(token_client.balance(&payee1), 293 + 243 + 98);
        assert_eq!(token_client.balance(&fee_collector), 9 + 14 + 2);

        // Now closed
        assert!(client.get_escrow(&1u128).closed);
    }

    #[test]
    fn test_timeout_and_auto_release() {
        let e = Env::default();
        e.mock_all_auths();
        e.ledger().set(Ledger { timestamp: 1000, protocol_version: 21, sequence_number: 1, network_passphrase: Default::default(), base_reserve: 0 });

        let admin = Address::generate(&e);
        let fee_collector = Address::generate(&e);
        let (contract_id, client) = deploy_contract(&e);
        let mut eadmins = SVec::new(&e); eadmins.push_back(Address::generate(&e));
        client.init(&admin, &0u32, &fee_collector, &eadmins, &1u32);

        let token_admin = Address::generate(&e);
        let token_addr = deploy_token(&e, &token_admin);
        let token_client = soroban_token_contract::Client::new(&e, &token_addr);
        
        let payer = Address::generate(&e);
        let payee1 = Address::generate(&e);
        let payee2 = Address::generate(&e);
        token_client.mint(&payer, &1000);

        let mut payers = SVec::new(&e); payers.push_back(payer.clone());
        let mut payees = SVec::new(&e); payees.push_back(payee1.clone()); payees.push_back(payee2.clone());
        let mut rels = SVec::new(&e); rels.push_back(payer.clone());
        let mut refs = SVec::new(&e); refs.push_back(payer.clone());
        let arbs = SVec::new(&e);

        client.create_escrow(&2u128, &EscrowInit { token: token_addr.clone(), payers: payers.clone(), payees: payees.clone(), release_signers: rels.clone(), release_threshold: 1, refund_signers: refs.clone(), refund_threshold: 1, arbiters: arbs.clone(), arbiter_threshold: 1, auto_release_ts: Some(1500), expiry_ts: 2000 });

        client.deposit(&2u128, &payer, &1000);

        // auto release at 1500
        e.ledger().set_timestamp(1500);
        client.auto_release(&2u128);
        // equal split 500/500, no fees
        assert_eq!(token_client.balance(&payee1), 500);
        assert_eq!(token_client.balance(&payee2), 500);

        // New escrow to test refund timeout
        client.create_escrow(&3u128, &EscrowInit { token: token_addr.clone(), payers: payers.clone(), payees: payees.clone(), release_signers: rels.clone(), release_threshold: 1, refund_signers: refs.clone(), refund_threshold: 1, arbiters: arbs.clone(), arbiter_threshold: 1, auto_release_ts: None, expiry_ts: 1200 });
        client.deposit(&3u128, &payer, &600);
        e.ledger().set_timestamp(1300);
        client.refund_timeout(&3u128);
        assert_eq!(token_client.balance(&payer), 1000 - 1000 + 600); // original balance after auto-release was 0, refunded 600
    }
}

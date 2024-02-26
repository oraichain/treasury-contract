use core::panic;
use cosmwasm_std::{to_binary, Addr, Event, Uint128};
use cw20::BalanceResponse;
use cw_multi_test::{error, App};

use crate::{state::DistributeTarget, ContractError};

use super::{
    contract::TreasuryContract,
    contract_ping_pong_mock::{Cw20Hook, MockPingPongContract},
    mock_cw20_contract::MockCw20Contract,
};

fn mock_app() -> (
    App,
    TreasuryContract,
    MockCw20Contract,
    MockPingPongContract,
) {
    let mut app = App::default();
    let owner = Addr::unchecked("owner");
    let finance = Addr::unchecked("finance");

    let cw20 = MockCw20Contract::instantiate(&mut app, &owner, &owner).unwrap();
    let ping_pong = MockPingPongContract::instantiate(&mut app, &owner);

    let treasury = TreasuryContract::instantiate(
        &mut app,
        &owner,
        &owner,
        cw20.addr(),
        Some(owner.clone().into()),
        vec![
            DistributeTarget {
                weight: 40,
                addr: ping_pong.addr().clone(),
                msg_hook: Some(to_binary(&Cw20Hook::Ping {}).unwrap()),
            },
            DistributeTarget {
                weight: 60,
                addr: finance,
                msg_hook: None,
            },
        ],
    )
    .unwrap();

    (app, treasury, cw20, ping_pong)
}

#[test]
fn test_distribute_happy_path() {
    let owner = Addr::unchecked("owner");
    let finance = Addr::unchecked("finance");
    let distribute_amount = Uint128::from(100u64);

    let (mut app, treasury, cw20, ping_pong) = mock_app();
    let owner_balance: BalanceResponse = cw20.query_balance(&app, &owner);

    println!("owner balance: {:?}", owner_balance);

    cw20.transfer(
        &mut app,
        &owner,
        &Addr::from(treasury.clone()),
        Uint128::from(100u64),
    );

    let treasury_balance: BalanceResponse = cw20.query_balance(&app, treasury.addr());
    assert_eq!(treasury_balance.balance, distribute_amount);

    let res = treasury
        .distribute_token(&owner, &mut app, distribute_amount)
        .unwrap();

    let ping_event = res
        .events
        .into_iter()
        .filter(|event| event.ty == "wasm" && event.attributes[1].value == "ping")
        .collect::<Vec<Event>>();

    // assert the ping event is emitted
    assert!(!ping_event.is_empty());

    let treasury_balance: BalanceResponse = cw20.query_balance(&app, treasury.addr());
    assert_eq!(treasury_balance.balance, Uint128::zero());
    let ping_pong_balance: BalanceResponse = cw20.query_balance(&app, ping_pong.addr());
    assert_eq!(ping_pong_balance.balance, Uint128::from(40u128));
    let finance: BalanceResponse = cw20.query_balance(&app, &finance);
    assert_eq!(finance.balance, Uint128::from(60u128));
}

#[test]
fn test_exceed_balance_distribute() {
    let owner = Addr::unchecked("owner");
    let _finance = Addr::unchecked("finance");
    let distribute_amount = Uint128::from(100u64);

    let (mut app, treasury, cw20, _ping_pong) = mock_app();
    let owner_balance: BalanceResponse = cw20.query_balance(&app, &owner);

    println!("owner balance: {:?}", owner_balance);

    cw20.transfer(
        &mut app,
        &owner,
        &Addr::from(treasury.clone()),
        Uint128::from(100u64),
    );

    let treasury_balance: BalanceResponse = cw20.query_balance(&app, treasury.addr());
    assert_eq!(treasury_balance.balance, distribute_amount);

    let err = treasury
        .distribute_token(
            &owner,
            &mut app,
            distribute_amount.checked_add(Uint128::from(1u64)).unwrap(),
        )
        .unwrap_err();

    assert_eq!(err, ContractError::ExceedContractBalance {});
}

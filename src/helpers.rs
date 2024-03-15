use cosmwasm_std::{Addr, Api, QuerierWrapper, StdResult, Uint128};
use oraiswap::{
    asset::AssetInfo,
    router::{self, SimulateSwapOperationsResponse, SwapOperation},
};

pub fn asset_info_from_string(api: &dyn Api, asset: String) -> AssetInfo {
    match api.addr_validate(&asset) {
        Ok(token_addr) => AssetInfo::Token {
            contract_addr: token_addr,
        },
        Err(_) => AssetInfo::NativeToken {
            denom: asset.to_string(),
        },
    }
}

pub fn calculate_minium_receive(
    querier: QuerierWrapper,
    router_address: Addr,
    offer_amount: Uint128,
    operations: Vec<SwapOperation>,
    slippage: Uint128,
) -> StdResult<Uint128> {
    let simulate_amount = querier
        .query_wasm_smart::<SimulateSwapOperationsResponse>(
            router_address,
            &router::QueryMsg::SimulateSwapOperations {
                offer_amount,
                operations: operations.clone(),
            },
        )?
        .amount;

    Ok(simulate_amount
        .checked_mul(Uint128::from(100u128) - slippage)?
        .checked_div(Uint128::from(100u128))?)
}

#[cfg(not(feature = "library"))]
use crate::msg::{ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, DistributeTarget, CONFIG, DISTRIBUTION_TARGETS};
use crate::ContractError;
use cosmwasm_std::{entry_point, to_binary, Addr, StdError, Uint128, WasmMsg};
use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;
use cw20::{BalanceResponse, Cw20ExecuteMsg};
use std::ops::Div;

// version info for migration info
const CONTRACT_NAME: &str = "crates.io:tresury";
const CONTRACT_VERSION: &str = env!("CARGO_PKG_VERSION");

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> Result<Response, ContractError> {
    set_contract_version(deps.storage, CONTRACT_NAME, CONTRACT_VERSION)?;

    let config = Config {
        owner: deps.api.addr_validate(msg.owner.as_str())?,
        distribute_token: msg.distribute_token,
    };

    CONFIG.save(deps.storage, &config)?;

    let valid_distribute_targets = msg
        .init_distribution_targets
        .iter()
        .map(|target| {
            Ok(DistributeTarget {
                weight: target.weight,
                addr: deps.api.addr_validate(target.addr.as_str())?,
                msg_hook: target.msg_hook.clone(),
            })
        })
        .collect::<Result<Vec<DistributeTarget>, ContractError>>()?;

    DISTRIBUTION_TARGETS.save(deps.storage, &valid_distribute_targets)?;
    Ok(Response::default())
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::UpdateConfig {
            owner,
            distribute_token,
        } => execute_update_config(deps, env, info, owner, distribute_token),
        ExecuteMsg::UpdateDistributeTarget { distribute_targets } => {
            execute_update_distribute_target(deps, env, info, distribute_targets)
        }
        ExecuteMsg::Distribute { amount_distribute } => {
            execute_distribute(deps, env, info, amount_distribute)
        }
    }
}

fn execute_update_config(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    owner: Option<Addr>,
    distribute_token: Option<Addr>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    if config.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    let new_config = Config {
        owner: owner.unwrap_or(config.owner),
        distribute_token: distribute_token.unwrap_or(config.distribute_token),
    };

    CONFIG.save(deps.storage, &new_config)?;

    Ok(Response::new()
        .add_attribute("action", "update_config")
        .add_attribute("owner", new_config.owner.as_str())
        .add_attribute("distribute_token", new_config.distribute_token.as_str()))
}

fn execute_update_distribute_target(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    distribute_targets: Vec<DistributeTarget>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if config.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    let valid_distribute_targets = distribute_targets
        .iter()
        .map(|target| {
            Ok(DistributeTarget {
                weight: target.weight,
                addr: deps.api.addr_validate(target.addr.as_str())?,
                msg_hook: target.msg_hook.clone(),
            })
        })
        .collect::<Result<Vec<DistributeTarget>, ContractError>>()?;

    DISTRIBUTION_TARGETS.save(deps.storage, &valid_distribute_targets)?;

    Ok(Response::new().add_attribute("action", "update_distribute_target"))
}

fn execute_distribute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    amount_distribute: Uint128,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;

    if config.owner != info.sender {
        return Err(ContractError::Unauthorized {});
    }

    let balance: BalanceResponse = deps.querier.query_wasm_smart(
        config.clone().distribute_token,
        &cw20_base::msg::QueryMsg::Balance {
            address: env.contract.address.into(),
        },
    )?;

    balance
        .balance
        .checked_sub(amount_distribute)
        .map_err(|_| ContractError::ExceedContractBalance {})?;

    let targets = DISTRIBUTION_TARGETS
        .load(deps.storage)?
        .iter()
        .map(|target| -> Result<WasmMsg, ContractError> {
            let transfer_amount = amount_distribute
                .checked_mul(Uint128::from(target.weight))
                .map_err(|e| ContractError::Std(StdError::Overflow { source: e }))?
                .div(Uint128::from(100u64));

            let msg = match target.clone().msg_hook {
                None => WasmMsg::Execute {
                    contract_addr: config.clone().distribute_token.into(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: target.clone().addr.into(),
                        amount: transfer_amount,
                    })?,
                    funds: vec![],
                },
                Some(msg_hook) => WasmMsg::Execute {
                    contract_addr: config.clone().distribute_token.into(),
                    msg: to_binary(&Cw20ExecuteMsg::Send {
                        contract: target.clone().addr.into(),
                        amount: transfer_amount,
                        msg: msg_hook,
                    })?,
                    funds: vec![],
                },
            };
            Ok(msg)
        })
        .collect::<Result<Vec<WasmMsg>, ContractError>>()?;

    Ok(Response::new()
        .add_messages(targets)
        .add_attribute("action", "distribute")
        .add_attribute("amount_distribute", amount_distribute.to_string()))
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(_deps: Deps, _env: Env, _msg: QueryMsg) -> StdResult<Binary> {
    unimplemented!()
}

#[cfg(test)]
mod tests {}

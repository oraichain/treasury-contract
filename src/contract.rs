#[cfg(not(feature = "library"))]
use crate::msg::{ConfigResponse, DistributeTargetsResponse, ExecuteMsg, InstantiateMsg, QueryMsg};
use crate::state::{Config, DistributeTarget, CONFIG, DISTRIBUTION_TARGETS};
use crate::ContractError;
use cosmwasm_std::{entry_point, to_binary, Addr, Decimal, Storage, Uint128, WasmMsg};
use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;
use cw20::{BalanceResponse, Cw20ExecuteMsg};

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
        config.distribute_token.clone(),
        &cw20_base::msg::QueryMsg::Balance {
            address: env.contract.address.into(),
        },
    )?;

    balance
        .balance
        .checked_sub(amount_distribute)
        .map_err(|_| ContractError::ExceedContractBalance {})?;

    let messages = _load_target_messages(deps.storage, amount_distribute, config.distribute_token)?;
    Ok(Response::new()
        .add_messages(messages)
        .add_attribute("action", "distribute")
        .add_attribute("amount_distribute", amount_distribute.to_string()))
}

fn _load_target_messages(
    storage: &mut dyn Storage,
    amount_distribute: Uint128,
    distribute_token: Addr,
) -> Result<Vec<WasmMsg>, ContractError> {
    DISTRIBUTION_TARGETS
        .load(storage)?
        .iter()
        .map(|target| -> Result<WasmMsg, ContractError> {
            let transfer_amount = amount_distribute * Decimal::percent(target.weight as u64);

            let msg = match target.clone().msg_hook {
                None => WasmMsg::Execute {
                    contract_addr: distribute_token.clone().into(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: target.clone().addr.into(),
                        amount: transfer_amount,
                    })?,
                    funds: vec![],
                },
                Some(msg_hook) => WasmMsg::Execute {
                    contract_addr: distribute_token.clone().into(),
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
        .collect::<Result<Vec<WasmMsg>, ContractError>>()
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::Config {} => to_binary(&ConfigResponse(CONFIG.load(deps.storage)?)),
        QueryMsg::DistributeTargets {} => to_binary(&DistributeTargetsResponse(
            DISTRIBUTION_TARGETS.load(deps.storage)?,
        )),
    }
}

#[cfg(test)]
mod tests {
    use crate::msg::{ConfigResponse, DistributeTargetsResponse, InstantiateMsg, QueryMsg};
    use crate::state::{Config, DistributeTarget};
    use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage};
    use cosmwasm_std::{from_binary, OwnedDeps, Uint128};
    use cosmwasm_std::{
        testing::{mock_dependencies, mock_env, mock_info},
        Addr,
    };

    use super::*;

    fn _instantiate_deps() -> OwnedDeps<MockStorage, MockApi, MockQuerier> {
        let mut deps = mock_dependencies();
        let init_distribution_targets = vec![
            DistributeTarget {
                weight: 40,
                addr: Addr::unchecked("target1"),
                msg_hook: Some(to_binary(&"hook1").unwrap()),
            },
            DistributeTarget {
                weight: 60,
                addr: Addr::unchecked("target2"),
                msg_hook: None,
            },
        ];

        let msg = InstantiateMsg {
            owner: Addr::unchecked("owner"),
            distribute_token: Addr::unchecked("distribute_token"),
            init_distribution_targets: init_distribution_targets.clone(),
        };

        let mock_info = mock_info("owner", &[]);
        let _res = instantiate(deps.as_mut(), mock_env(), mock_info, msg).unwrap();

        let config_binary = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
        let config = from_binary::<ConfigResponse>(&config_binary).unwrap();

        assert_eq!(
            config,
            ConfigResponse(Config {
                owner: Addr::unchecked("owner"),
                distribute_token: Addr::unchecked("distribute_token"),
            })
        );

        let distribute_targets_binary =
            query(deps.as_ref(), mock_env(), QueryMsg::DistributeTargets {}).unwrap();

        let distribute_targets =
            from_binary::<DistributeTargetsResponse>(&distribute_targets_binary).unwrap();

        assert_eq!(
            distribute_targets,
            DistributeTargetsResponse(init_distribution_targets)
        );

        // send token

        deps
    }

    #[test]
    fn test_instantiate() {
        let _deps = _instantiate_deps();
    }

    #[test]
    fn test_load_target_messages() {
        let mut deps = _instantiate_deps();

        let amount_distribute = Uint128::from(1000u128);
        let distribute_token = Addr::unchecked("distribute_token");

        let messages =
            _load_target_messages(&mut deps.storage, amount_distribute, distribute_token).unwrap();

        assert_eq!(
            messages,
            vec![
                WasmMsg::Execute {
                    contract_addr: "distribute_token".into(),
                    msg: to_binary(&Cw20ExecuteMsg::Send {
                        contract: "target1".into(),
                        amount: Uint128::from(400u64),
                        msg: to_binary(&"hook1").unwrap()
                    })
                    .unwrap(),
                    funds: vec![]
                },
                WasmMsg::Execute {
                    contract_addr: "distribute_token".into(),
                    msg: to_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: "target2".into(),
                        amount: Uint128::from(600u64)
                    })
                    .unwrap(),
                    funds: vec![]
                }
            ]
        )
    }
}

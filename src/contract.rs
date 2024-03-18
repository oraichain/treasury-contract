#[cfg(not(feature = "library"))]
use crate::helpers::asset_info_from_string;
use crate::msg::{
    CollectFeeRequirement, ConfigResponse, DistributeTargetsResponse, ExecuteMsg, InstantiateMsg,
    MigrateMsg, QueryMsg,
};
use crate::state::{Config, DistributeTarget, CONFIG, DISTRIBUTION_TARGETS};
use crate::ContractError;
use cosmos_sdk_proto::cosmos::authz::v1beta1::MsgExec;
use cosmos_sdk_proto::cosmos::bank::v1beta1::MsgSend;
use cosmos_sdk_proto::cosmos::base::v1beta1::Coin;
use cosmos_sdk_proto::traits::{Message, MessageExt};

use cosmwasm_std::{
    entry_point, to_json_binary, Addr, CosmosMsg, Decimal, Storage, Uint128, WasmMsg,
};
use cosmwasm_std::{Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult};
use cw2::set_contract_version;
use cw20::{BalanceResponse, Cw20ExecuteMsg};
use oraiswap::asset::AssetInfo;
use oraiswap::router::{
    Cw20HookMsg as Cw20RouterHookMsg, ExecuteMsg as RouterExecuteMsg, SwapOperation,
};

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
        router: match msg.router {
            Some(addr) => Some(deps.api.addr_validate(addr.as_str())?),
            None => None,
        },
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
        ExecuteMsg::CollectFees {
            collect_fee_requirements,
        } => execute_collect_fees(deps, env, info, collect_fee_requirements),
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
        router: config.router,
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

pub fn execute_collect_fees(
    deps: DepsMut,
    env: Env,
    _info: MessageInfo,
    collect_fee_requirements: Vec<CollectFeeRequirement>,
) -> Result<Response, ContractError> {
    let config = CONFIG.load(deps.storage)?;
    let mut messages: Vec<CosmosMsg> = vec![];
    if config.router.is_none() {
        return Err(ContractError::RouterAndApproverNotSet {});
    }
    let router_unwrap = config.router.unwrap();
    // create a new variable for better code readability
    let fees_receiver = env.contract.address;
    // build swap operations
    let approver_messages = collect_fee_requirements
        .iter()
        .map(|requirement| -> StdResult<Option<Vec<CosmosMsg>>> {
            let operations = requirement.clone().swap_operations;

            let offer_asset = match &operations[0] {
                SwapOperation::OraiSwap {
                    offer_asset_info, ..
                } => offer_asset_info,
            };

            let distribute_asset_info =
                asset_info_from_string(deps.api, config.distribute_token.clone().into());

            let final_ask_asset = match &operations[operations.len() - 1] {
                SwapOperation::OraiSwap { ask_asset_info, .. } => ask_asset_info,
            };

            // final ask asset should be distribute token
            if distribute_asset_info != final_ask_asset.clone() {
                return Ok(None);
            }

            let balance = offer_asset
                .query_pool(&deps.querier, requirement.approver.clone())
                .unwrap_or_default();

            if balance.is_zero() {
                return Ok(None);
            }
            // Assume that the owner approve infinite allowance to the contract
            match &offer_asset {
                AssetInfo::Token { contract_addr } => {
                    // transfer from only if distribute asset equals to offer_asset
                    if distribute_asset_info == offer_asset.clone() {
                        return Ok(Some(vec![CosmosMsg::Wasm(WasmMsg::Execute {
                            contract_addr: contract_addr.clone().into(),
                            msg: to_json_binary(&Cw20ExecuteMsg::TransferFrom {
                                owner: requirement.approver.to_string(),
                                recipient: fees_receiver.to_string(),
                                amount: balance,
                            })?,
                            funds: vec![],
                        })]));
                    }

                    Ok(Some(vec![CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: contract_addr.clone().into(),
                        msg: to_json_binary(&Cw20ExecuteMsg::SendFrom {
                            owner: requirement.approver.to_string(),
                            contract: router_unwrap.to_string(),
                            amount: balance,
                            msg: to_json_binary(&Cw20RouterHookMsg::ExecuteSwapOperations {
                                operations,
                                minimum_receive: requirement.minimum_receive,
                                to: Some(fees_receiver.to_string()),
                            })?,
                        })?,
                        funds: vec![],
                    })]))
                }
                // handle native token
                AssetInfo::NativeToken { denom } => {
                    let mut swap_amount = balance;
                    if denom == "orai" {
                        // Left 1 orai for transaction fee
                        swap_amount = swap_amount
                            .checked_sub(Uint128::from(1000000u128))
                            .unwrap_or_default();
                    }

                    if swap_amount.is_zero() {
                        return Ok(None);
                    }

                    let send = MsgSend {
                        from_address: requirement.approver.to_string(),
                        to_address: fees_receiver.to_string(),
                        amount: vec![Coin {
                            denom: denom.clone(),
                            amount: swap_amount.to_string(),
                        }],
                    };
                    let send_any_result = send.to_any();
                    if send_any_result.is_err() {
                        return Ok(None);
                    }

                    let stargate_value = Binary::from(
                        MsgExec {
                            grantee: fees_receiver.to_string(),
                            msgs: vec![send_any_result.unwrap()],
                        }
                        .encode_to_vec(),
                    );
                    // transfer_from native token
                    let stargate = CosmosMsg::Stargate {
                        type_url: "/cosmos.authz.v1beta1.MsgExec".to_string(),
                        value: stargate_value,
                    };

                    let wasm_swap = CosmosMsg::Wasm(WasmMsg::Execute {
                        contract_addr: router_unwrap.to_string(),
                        msg: to_json_binary(&RouterExecuteMsg::ExecuteSwapOperations {
                            operations: operations.clone(),
                            to: Some(fees_receiver.clone()),
                            minimum_receive: requirement.minimum_receive,
                        })?,
                        funds: vec![cosmwasm_std::Coin {
                            denom: denom.clone(),
                            amount: swap_amount,
                        }],
                    });

                    Ok(Some(vec![stargate, wasm_swap]))
                }
            }
        })
        .filter_map(|msgs| match msgs {
            Ok(Some(cosmos_msgs)) => Some(cosmos_msgs),
            _ => None,
        })
        .flatten()
        .collect::<Vec<CosmosMsg>>();

    messages.extend(approver_messages);

    let mut response = Response::new();

    if !messages.is_empty() {
        response = response.add_messages(messages);
    }

    Ok(response)
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
                    msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: target.clone().addr.into(),
                        amount: transfer_amount,
                    })?,
                    funds: vec![],
                },
                Some(msg_hook) => WasmMsg::Execute {
                    contract_addr: distribute_token.clone().into(),
                    msg: to_json_binary(&Cw20ExecuteMsg::Send {
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
        QueryMsg::Config {} => to_json_binary(&ConfigResponse(CONFIG.load(deps.storage)?)),
        QueryMsg::DistributeTargets {} => to_json_binary(&DistributeTargetsResponse(
            DISTRIBUTION_TARGETS.load(deps.storage)?,
        )),
    }
}

#[cfg_attr(not(feature = "library"), entry_point)]
pub fn migrate(_deps: DepsMut, _env: Env, _msg: MigrateMsg) -> Result<Response, ContractError> {
    Ok(Response::default())
}

#[cfg(test)]
mod tests {
    use crate::msg::{ConfigResponse, DistributeTargetsResponse, InstantiateMsg, QueryMsg};
    use crate::state::{Config, DistributeTarget};
    use cosmwasm_std::testing::{MockApi, MockQuerier, MockStorage};
    use cosmwasm_std::{from_json, OwnedDeps, Uint128};
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
                msg_hook: Some(to_json_binary(&"hook1").unwrap()),
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
            router: Some(Addr::unchecked("router")),
        };

        let mock_info = mock_info("owner", &[]);
        let _res = instantiate(deps.as_mut(), mock_env(), mock_info, msg).unwrap();

        let config_binary = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
        let config = from_json::<ConfigResponse>(&config_binary).unwrap();

        assert_eq!(
            config,
            ConfigResponse(Config {
                owner: Addr::unchecked("owner"),
                distribute_token: Addr::unchecked("distribute_token"),
                router: Some(Addr::unchecked("router")),
            })
        );

        let distribute_targets_binary =
            query(deps.as_ref(), mock_env(), QueryMsg::DistributeTargets {}).unwrap();

        let distribute_targets =
            from_json::<DistributeTargetsResponse>(&distribute_targets_binary).unwrap();

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
                    msg: to_json_binary(&Cw20ExecuteMsg::Send {
                        contract: "target1".into(),
                        amount: Uint128::from(400u64),
                        msg: to_json_binary(&"hook1").unwrap()
                    })
                    .unwrap(),
                    funds: vec![]
                },
                WasmMsg::Execute {
                    contract_addr: "distribute_token".into(),
                    msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: "target2".into(),
                        amount: Uint128::from(600u64)
                    })
                    .unwrap(),
                    funds: vec![]
                }
            ]
        )
    }

    #[test]
    fn test_execute_update_config() {
        let mut deps = _instantiate_deps();

        let msg = ExecuteMsg::UpdateConfig {
            owner: Some(Addr::unchecked("new_owner")),
            distribute_token: Some(Addr::unchecked("new_distribute_token")),
        };

        // act
        execute(deps.as_mut(), mock_env(), mock_info("owner", &[]), msg).unwrap();

        let raw_config = query(deps.as_ref(), mock_env(), QueryMsg::Config {}).unwrap();
        let config = from_json::<ConfigResponse>(&raw_config).unwrap();

        // assert
        assert_eq!(config.0.owner, Addr::unchecked("new_owner"));
        assert_eq!(
            config.0.distribute_token,
            Addr::unchecked("new_distribute_token")
        );
    }

    #[test]
    fn test_execute_update_distribute_targets() {
        let mut deps = _instantiate_deps();

        let init_distribution_targets = vec![
            DistributeTarget {
                weight: 40,
                addr: Addr::unchecked("target3"),
                msg_hook: Some(to_json_binary(&"hook1").unwrap()),
            },
            DistributeTarget {
                weight: 60,
                addr: Addr::unchecked("target4"),
                msg_hook: None,
            },
        ];

        // act
        execute(
            deps.as_mut(),
            mock_env(),
            mock_info("owner", &[]),
            ExecuteMsg::UpdateDistributeTarget {
                distribute_targets: init_distribution_targets.clone(),
            },
        )
        .unwrap();

        let raw_distribute_targets =
            query(deps.as_ref(), mock_env(), QueryMsg::DistributeTargets {}).unwrap();

        let distribute_targets = from_json::<DistributeTargetsResponse>(&raw_distribute_targets)
            .unwrap()
            .0;

        // assert
        assert_eq!(distribute_targets[0], init_distribution_targets[0]);
        assert_eq!(distribute_targets[1], init_distribution_targets[1]);
    }

    #[test]
    fn test_function_authorize() {
        let mut deps = _instantiate_deps();

        // act
        let distribute_unauthorized_err = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("not_owner", &[]),
            ExecuteMsg::Distribute {
                amount_distribute: Uint128::from(1000u128),
            },
        )
        .unwrap_err();

        let update_config_unauthorized_err = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("not_owner", &[]),
            ExecuteMsg::UpdateConfig {
                owner: None,
                distribute_token: None,
            },
        )
        .unwrap_err();

        let update_distribute_target_unauthorized_err = execute(
            deps.as_mut(),
            mock_env(),
            mock_info("not_owner", &[]),
            ExecuteMsg::UpdateDistributeTarget {
                distribute_targets: vec![],
            },
        )
        .unwrap_err();

        // assert
        assert_eq!(distribute_unauthorized_err, ContractError::Unauthorized {});
        assert_eq!(
            update_config_unauthorized_err,
            ContractError::Unauthorized {}
        );
        assert_eq!(
            update_distribute_target_unauthorized_err,
            ContractError::Unauthorized {}
        );
    }
}

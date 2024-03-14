use cosmwasm_schema::cw_serde;
use cosmwasm_std::{
    to_binary, Addr, Binary, DepsMut, Env, MessageInfo, Response, StdResult, WasmMsg,
};
use cw20::{Cw20ExecuteMsg, Cw20ReceiveMsg};
use cw_multi_test::{ContractWrapper, Executor};
use cw_storage_plus::Item;
use oraiswap::router::ExecuteMsg as RouterExecuteMsg;

use super::tests::StargateAccpetingModuleApp;

#[cw_serde]
pub enum MockQueryMsg {}

#[cw_serde]
pub enum Cw20Hook {
    Ping {},
}

#[cw_serde]
pub struct MockInstantiateMsg {
    pub usdc: Addr,
}

#[cw_serde]
pub struct MockRouter(Addr);

/**
 * MockRouter is a mock contract that implements the Router interface.
 * It is used to test the integration of the Router with other contracts.
 */
const USDC: Item<Addr> = Item::new("usdc");
fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: MockInstantiateMsg,
) -> StdResult<Response> {
    USDC.save(deps.storage, &msg.usdc)?;
    Ok(Response::default())
}
impl MockRouter {
    pub fn addr(&self) -> &Addr {
        &self.0
    }

    pub fn store_code(app: &mut StargateAccpetingModuleApp) -> u64 {
        let contract = ContractWrapper::new(
            |deps: DepsMut, _, info: MessageInfo, msg: RouterExecuteMsg| -> StdResult<Response> {
                // swap 1:1 (cw20-> usdc, orai -> usdc)
                match msg {
                    RouterExecuteMsg::Receive(Cw20ReceiveMsg {
                        sender,
                        amount,
                        msg,
                    }) => {
                        // return usdc to sender
                        let msg = WasmMsg::Execute {
                            contract_addr: USDC.load(deps.storage)?.to_string(),
                            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                                recipient: sender.to_string(),
                                amount: amount,
                            })?,
                            funds: vec![],
                        };
                        Ok(Response::new()
                            .add_message(msg)
                            .add_attribute("action", "execute_swap_operations")
                            .add_attribute("trader", sender.to_string())
                            .add_attribute("amount", amount.to_string()))
                    }
                    RouterExecuteMsg::ExecuteSwapOperations {
                        operations,
                        minimum_receive,
                        to,
                    } => {
                        // return usdc to sender
                        let msg = WasmMsg::Execute {
                            contract_addr: USDC.load(deps.storage)?.to_string(),
                            msg: to_binary(&Cw20ExecuteMsg::Transfer {
                                recipient: to.unwrap().to_string(),
                                amount: info.funds[0].amount,
                            })?,
                            funds: vec![],
                        };
                        Ok(Response::new()
                            .add_message(msg)
                            .add_attribute("action", "execute_swap_operations")
                            .add_attribute("trader", info.sender.to_string())
                            .add_attribute("amount", info.funds[0].amount.to_string()))
                    }

                    _ => Ok(Response::default()),
                }
            },
            instantiate,
            |_, _, _: MockQueryMsg| -> StdResult<Binary> { Ok(Binary::default()) },
        );
        app.store_code(Box::new(contract))
    }

    pub fn instantiate(app: &mut StargateAccpetingModuleApp, sender: &Addr, usdc: Addr) -> Self {
        let code_id = Self::store_code(app);
        let contract_addr = app
            .instantiate_contract(
                code_id,
                sender.clone(),
                &MockInstantiateMsg { usdc },
                &[],
                "ping_pong",
                None,
            )
            .unwrap();

        MockRouter(contract_addr)
    }
}

impl From<MockRouter> for Addr {
    fn from(contract: MockRouter) -> Self {
        contract.0
    }
}

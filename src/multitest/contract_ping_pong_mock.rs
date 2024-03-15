use super::tests::StargateAccpetingModuleApp;
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Binary, Response, StdResult};
use cw20::Cw20ReceiveMsg;
use cw_multi_test::{ContractWrapper, Executor};

#[cw_serde]
pub enum MockExecuteMsg {
    Receive(Cw20ReceiveMsg),
}

#[cw_serde]
pub enum MockQueryMsg {}

#[cw_serde]
pub enum Cw20Hook {
    Ping {},
}

#[cw_serde]
pub struct MockInstantiateMsg {}

#[cw_serde]
pub struct MockPingPongContract(Addr);

impl MockPingPongContract {
    pub fn addr(&self) -> &Addr {
        &self.0
    }

    pub fn store_code(app: &mut StargateAccpetingModuleApp) -> u64 {
        let contract = ContractWrapper::new(
            |_, _, _, msg: MockExecuteMsg| -> StdResult<Response> {
                match msg {
                    MockExecuteMsg::Receive(_) => {
                        Ok(Response::new().add_attribute("action", "ping"))
                    }
                }
            },
            |_, _, _, _: MockInstantiateMsg| -> StdResult<Response> { Ok(Response::default()) },
            |_, _, _: MockQueryMsg| -> StdResult<Binary> { Ok(Binary::default()) },
        );
        app.store_code(Box::new(contract))
    }

    pub fn instantiate(app: &mut StargateAccpetingModuleApp, sender: &Addr) -> Self {
        let code_id = Self::store_code(app);
        let contract_addr = app
            .instantiate_contract(
                code_id,
                sender.clone(),
                &MockInstantiateMsg {},
                &[],
                "ping_pong",
                None,
            )
            .unwrap();

        MockPingPongContract(contract_addr)
    }
}

impl From<MockPingPongContract> for Addr {
    fn from(contract: MockPingPongContract) -> Self {
        contract.0
    }
}

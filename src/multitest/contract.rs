use crate::msg::{ExecuteMsg, InstantiateMsg};
use crate::{error::ContractError, state::DistributeTarget};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_multi_test::{AppResponse, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, migrate, query};

use super::tests::StargateAccpetingModuleApp;

#[cw_serde]
pub struct TreasuryContract(Addr);

impl TreasuryContract {
    pub fn addr(&self) -> &Addr {
        &self.0
    }

    pub fn store_code(app: &mut StargateAccpetingModuleApp) -> u64 {
        let contract = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
        app.store_code(Box::new(contract))
    }

    #[track_caller]
    pub fn instantiate(
        app: &mut StargateAccpetingModuleApp,
        sender: &Addr,
        owner: &Addr,
        distribute_token: &Addr,
        admin: Option<String>,
        router: &Addr,
        init_distribution_targets: Vec<DistributeTarget>,
    ) -> Result<Self, ContractError> {
        let code_id = Self::store_code(app);
        app.instantiate_contract(
            code_id,
            sender.clone(),
            &InstantiateMsg {
                owner: owner.clone(),
                distribute_token: distribute_token.clone(),
                init_distribution_targets,
                router: Some(router.clone()),
                executors: vec![owner.clone()],
            },
            &[],
            "treasury contract",
            admin,
        )
        .map(TreasuryContract)
        .map_err(|err| err.downcast().unwrap())
    }

    #[track_caller]
    pub fn distribute_token(
        &self,
        sender: &Addr,
        app: &mut StargateAccpetingModuleApp,
        amount_distribute: Uint128,
    ) -> Result<AppResponse, ContractError> {
        app.execute_contract(
            sender.clone(),
            self.0.clone(),
            &ExecuteMsg::Distribute { amount_distribute },
            &[],
        )
        .map_err(|err| err.downcast().unwrap())
    }
}

impl From<TreasuryContract> for Addr {
    fn from(contract: TreasuryContract) -> Addr {
        contract.0
    }
}

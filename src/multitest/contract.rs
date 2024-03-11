use crate::msg::{ExecuteMsg, InstantiateMsg};
use crate::{error::ContractError, state::DistributeTarget};
use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Uint128};
use cw_multi_test::{App, AppResponse, ContractWrapper, Executor};

use crate::contract::{execute, instantiate, migrate, query};

#[cw_serde]
pub struct TreasuryContract(Addr);

impl TreasuryContract {
    pub fn addr(&self) -> &Addr {
        &self.0
    }

    pub fn store_code(app: &mut App) -> u64 {
        let contract = ContractWrapper::new(execute, instantiate, query).with_migrate(migrate);
        app.store_code(Box::new(contract))
    }

    #[track_caller]
    pub fn instantiate(
        app: &mut App,
        sender: &Addr,
        owner: &Addr,
        distribute_token: &Addr,
        admin: Option<String>,
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
                approver: Some(vec![Addr::unchecked(admin.clone().unwrap())]),
                router: Some(Addr::unchecked("router")),
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
        app: &mut App,
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

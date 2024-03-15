use cosmwasm_schema::cw_serde;
use cosmwasm_std::{Addr, Binary};
use cw_storage_plus::Item;

const CONFIG_KEY: &str = "config";
const DISTRIBUTION_TARGET: &str = "distribution_target";

#[cw_serde]
pub struct Config {
    pub owner: Addr,
    pub distribute_token: Addr,
    pub router: Option<Addr>,
    pub approver: Option<Vec<Addr>>,
}

#[cw_serde]
pub struct DistributeTarget {
    pub addr: Addr,
    pub weight: u32, // total weight distribute target should be 100
    pub msg_hook: Option<Binary>,
}

pub const CONFIG: Item<Config> = Item::new(CONFIG_KEY);
pub const DISTRIBUTION_TARGETS: Item<Vec<DistributeTarget>> = Item::new(DISTRIBUTION_TARGET);

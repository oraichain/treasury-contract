use cosmwasm_std::Api;
use oraiswap::asset::AssetInfo;

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

use astroport::asset::{Asset, AssetInfo, AssetInfoExt};
use cosmwasm_std::StdResult;

pub struct Assets(pub Vec<Asset>);

impl Assets {
    pub fn add(&mut self, coin_to_add: &Asset) -> StdResult<()> {
        match self.0.iter_mut().find(|coin| coin.info == coin_to_add.info) {
            Some(coin) => {
                coin.amount = coin.amount.checked_add(coin_to_add.amount)?;
            },
            None => {
                self.0.push(coin_to_add.clone());
            },
        }
        Ok(())
    }

    pub fn add_many(&mut self, coins_to_add: &Assets) -> StdResult<()> {
        for coin_to_add in &coins_to_add.0 {
            self.add(coin_to_add)?;
        }
        Ok(())
    }

    pub fn find(&self, info: &AssetInfo) -> Asset {
        self.0
            .iter()
            .cloned()
            .find(|coin| coin.info == *info)
            .unwrap_or_else(|| info.with_balance(0u128))
    }
}

use anyhow::Result;
use async_trait::async_trait;
use snarkvm::ledger::query::QueryTrait;
use snarkvm::prelude::{Field, Network, StatePath};
use std::str::FromStr;

/// Custom query that returns a fixed state root.
///
/// Bypasses snarkVM's internal `ureq` (blocked by Cloudflare WAF) and ensures
/// both execution and fee traces use the exact same state root.
pub(crate) struct FixedStateRootQuery<N: Network> {
    pub(crate) state_root: N::StateRoot,
    pub(crate) block_height: u32,
}

#[async_trait(?Send)]
impl<N: Network> QueryTrait<N> for FixedStateRootQuery<N> {
    fn current_state_root(&self) -> Result<N::StateRoot> {
        Ok(self.state_root.clone())
    }

    fn current_block_height(&self) -> Result<u32> {
        Ok(self.block_height)
    }

    fn get_state_path_for_commitment(&self, _commitment: &Field<N>) -> Result<StatePath<N>> {
        StatePath::from_str("").or_else(|_| anyhow::bail!("State path not available"))
    }

    fn get_state_paths_for_commitments(&self, _commitments: &[Field<N>]) -> Result<Vec<StatePath<N>>> {
        Ok(Vec::new())
    }

    async fn current_state_root_async(&self) -> Result<N::StateRoot> {
        Ok(self.state_root.clone())
    }

    async fn current_block_height_async(&self) -> Result<u32> {
        Ok(self.block_height)
    }

    async fn get_state_path_for_commitment_async(&self, _commitment: &Field<N>) -> Result<StatePath<N>> {
        StatePath::from_str("").or_else(|_| anyhow::bail!("State path not available"))
    }

    async fn get_state_paths_for_commitments_async(&self, _commitments: &[Field<N>]) -> Result<Vec<StatePath<N>>> {
        Ok(Vec::new())
    }
}

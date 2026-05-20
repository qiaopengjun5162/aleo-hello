use anyhow::{Context, Result};
use snarkvm::algorithms::snark::varuna::VarunaVersion;
use snarkvm::circuit::AleoTestnetV0;
use snarkvm::ledger::block::Transaction;
use snarkvm::parameters::testnet::{FeePublicV0Prover, FeePublicV0Verifier};
use snarkvm::console::program::ProgramID;
use snarkvm::prelude::{
    ConsensusVersion, Identifier, InclusionVersion, PrivateKey, Process, Program,
    Response, TestRng, TestnetV0,
};
use snarkvm::prelude::FromBytes as _;
use snarkvm::synthesizer::snark::{ProvingKey, VerifyingKey};
use snarkvm::synthesizer::process::Trace;
use std::str::FromStr;

use crate::query::FixedStateRootQuery;

/// Holds the proving engine state.
pub(crate) struct Engine {
    pub(crate) process: Process<TestnetV0>,
}

impl Engine {
    /// Initialize process, load credits + user program, inject V0 fee keys.
    pub(crate) fn init(program: &Program<TestnetV0>) -> Result<Self> {
        let mut process = Process::<TestnetV0>::load()?;

        let credits_program = Program::<TestnetV0>::credits()?;
        process.add_program(&credits_program)?;
        process.add_program(program)?;

        // Inject V0 fee keys — deployed program uses edition 0 / V0 credits
        println!("⏳ Loading V0 fee keys from testnet parameters...");
        let fee_pk = ProvingKey::<TestnetV0>::from_bytes_le(
            &FeePublicV0Prover::load_bytes()?
        ).context("Failed to deserialize V0 fee proving key")?;
        let fee_vk = VerifyingKey::<TestnetV0>::from_bytes_le(
            &FeePublicV0Verifier::load_bytes()?
        ).context("Failed to deserialize V0 fee verifying key")?;

        let credits_id = credits_program.id();
        let fee_fn = Identifier::<TestnetV0>::from_str("fee_public")?;
        process.insert_proving_key(credits_id, &fee_fn, fee_pk)?;
        process.insert_verifying_key(credits_id, &fee_fn, fee_vk)?;
        println!("✅ V0 Fee keys injected into VM");

        Ok(Self { process })
    }

    /// Authorize + execute locally. Returns the plaintext response and execution trace.
    pub(crate) fn authorize_and_execute(
        &mut self,
        private_key: &PrivateKey<TestnetV0>,
        program_id: &ProgramID<TestnetV0>,
        function_name: &str,
        inputs: Vec<&str>,
        rng: &mut TestRng,
    ) -> Result<(Response<TestnetV0>, Trace<TestnetV0>)> {
        println!("⏳ Phase 1: Authorizing...");
        let authorization = self.process.authorize::<AleoTestnetV0, _>(
            private_key, *program_id, function_name, inputs.into_iter(), rng,
        ).context("Authorization failed")?;
        println!("✅ Authorization generated");

        println!("⏳ Phase 2: Local execution...");
        let (response, trace) = self.process.execute::<AleoTestnetV0, _>(authorization, rng)
            .context("Local execution failed")?;

        println!("=======================================================");
        println!("🌟 Execution Response");
        println!("=======================================================");
        for (i, output) in response.outputs().iter().enumerate() {
            println!("  Output [{}]: {}", i, output);
        }
        println!("=======================================================\n");

        Ok((response, trace))
    }

    /// Prove execution + fee, verify locally, package into Transaction.
    pub(crate) fn prove_and_package(
        &mut self,
        trace: Trace<TestnetV0>,
        private_key: &PrivateKey<TestnetV0>,
        program_id: &ProgramID<TestnetV0>,
        function_name: &str,
        base_fee: u64,
        priority_fee: u64,
        query: &FixedStateRootQuery<TestnetV0>,
        rng: &mut TestRng,
    ) -> Result<Transaction<TestnetV0>> {
        let locator = format!("{}/{}", program_id, function_name);

        // Prove execution
        let mut exec_trace = trace;
        exec_trace.prepare(query).context("Failed to prepare execution trace")?;
        let execution = exec_trace.prove_execution::<AleoTestnetV0, _>(&locator, VarunaVersion::V2, rng)
            .context("Failed to generate execution proof")?;

        // Fee authorization (needs execution_id)
        let execution_id = execution.to_execution_id()?;
        let fee_authorization = self.process.authorize_fee_public::<AleoTestnetV0, _>(
            private_key, base_fee, priority_fee, execution_id, rng,
        ).context("Failed to authorize fee")?;

        let (_fee_response, mut fee_trace) = self.process
            .execute::<AleoTestnetV0, _>(fee_authorization, rng)
            .context("Failed to execute fee")?;

        fee_trace.prepare(query).context("Failed to prepare fee trace")?;
        let fee = fee_trace.prove_fee::<AleoTestnetV0, _>(VarunaVersion::V2, rng)
            .context("Failed to generate fee proof")?;

        // Local verification
        println!("🔍 Verifying proofs locally...");
        self.process.verify_execution(
            ConsensusVersion::V14, VarunaVersion::V2, InclusionVersion::V0, &execution,
        ).context("Local execution verification FAILED")?;
        println!("  ✅ Execution proof verified");

        self.process.verify_fee(
            ConsensusVersion::V14, VarunaVersion::V2, InclusionVersion::V0, &fee, execution_id,
        ).context("Local fee verification FAILED")?;
        println!("  ✅ Fee proof verified");

        Transaction::<TestnetV0>::from_execution(execution, Some(fee))
            .context("Failed to package transaction")
    }
}

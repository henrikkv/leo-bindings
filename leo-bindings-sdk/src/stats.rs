use colored::*;
use num_format::{Locale, ToFormattedString};
use snarkvm::{
    ledger::store::helpers::memory::ConsensusMemory,
    prelude::{
        ConsensusVersion, Deployment, Execution, Network, Result, VM, deployment_cost,
        execution_cost,
    },
};

/// Pretty-print deployment statistics.
pub fn print_deployment_stats<N: Network>(
    vm: &VM<N, ConsensusMemory<N>>,
    program_id: &str,
    deployment: &Deployment<N>,
    priority_fee: Option<u64>,
    consensus_version: ConsensusVersion,
) -> Result<()> {
    let variables = deployment.num_combined_variables()?;
    let constraints = deployment.num_combined_constraints()?;
    let (base_fee, (storage_cost, synthesis_cost, constructor_cost, namespace_cost)) =
        deployment_cost(&vm.process().read(), deployment, consensus_version)?;

    let base_fee_cr = base_fee as f64 / 1_000_000.0;
    let prio_fee_cr = priority_fee.unwrap_or(0) as f64 / 1_000_000.0;
    let total_fee_cr = base_fee_cr + prio_fee_cr;

    log::info!(
        "\n{} {}",
        "📊 Deployment Summary for".bold(),
        program_id.bold()
    );
    log::info!(
        "{}",
        "──────────────────────────────────────────────".dimmed()
    );

    log::info!(
        "  {:22}{}",
        "Total Variables:".cyan(),
        variables.to_formatted_string(&Locale::en).yellow()
    );
    log::info!(
        "  {:22}{}",
        "Total Constraints:".cyan(),
        constraints.to_formatted_string(&Locale::en).yellow()
    );
    log::info!(
        "  {:22}{}",
        "Max Variables:".cyan(),
        N::MAX_DEPLOYMENT_VARIABLES
            .to_formatted_string(&Locale::en)
            .green()
    );
    log::info!(
        "  {:22}{}",
        "Max Constraints:".cyan(),
        N::MAX_DEPLOYMENT_CONSTRAINTS
            .to_formatted_string(&Locale::en)
            .green()
    );

    log::info!("\n{}", "💰 Cost Breakdown (credits)".bold());
    log::info!(
        "  {:22}{}{:.6}",
        "Transaction Storage:".cyan(),
        "".yellow(),
        storage_cost as f64 / 1_000_000.0
    );
    log::info!(
        "  {:22}{}{:.6}",
        "Program Synthesis:".cyan(),
        "".yellow(),
        synthesis_cost as f64 / 1_000_000.0
    );
    log::info!(
        "  {:22}{}{:.6}",
        "Namespace:".cyan(),
        "".yellow(),
        namespace_cost as f64 / 1_000_000.0
    );
    log::info!(
        "  {:22}{}{:.6}",
        "Constructor:".cyan(),
        "".yellow(),
        constructor_cost as f64 / 1_000_000.0
    );
    log::info!(
        "  {:22}{}{:.6}",
        "Priority Fee:".cyan(),
        "".yellow(),
        prio_fee_cr
    );
    log::info!(
        "  {:22}{}{:.6}",
        "Total Fee:".cyan(),
        "".yellow(),
        total_fee_cr
    );

    log::info!(
        "{}",
        "──────────────────────────────────────────────".dimmed()
    );

    if variables > N::MAX_DEPLOYMENT_VARIABLES {
        return Err(snarkvm::prelude::Error::msg(format!(
            "Deployment exceeds maximum variables: {} > {}",
            variables,
            N::MAX_DEPLOYMENT_VARIABLES
        )));
    }

    if constraints > N::MAX_DEPLOYMENT_CONSTRAINTS {
        return Err(snarkvm::prelude::Error::msg(format!(
            "Deployment exceeds maximum constraints: {} > {}",
            constraints,
            N::MAX_DEPLOYMENT_CONSTRAINTS
        )));
    }

    Ok(())
}

/// Pretty-print execution statistics.
pub fn print_execution_stats<N: Network>(
    vm: &VM<N, ConsensusMemory<N>>,
    program_name: &str,
    execution: &Execution<N>,
    priority_fee: Option<u64>,
    consensus_version: ConsensusVersion,
) -> Result<()> {
    let (base_fee, (storage_cost, execution_cost)) =
        execution_cost(&vm.process().read(), execution, consensus_version)?;

    let base_cr = base_fee as f64 / 1_000_000.0;
    let prio_cr = priority_fee.unwrap_or(0) as f64 / 1_000_000.0;
    let total_cr = base_cr + prio_cr;

    log::info!(
        "\n{} {}",
        "📊 Execution Summary for".bold(),
        program_name.bold()
    );
    log::info!(
        "{}",
        "──────────────────────────────────────────────".dimmed()
    );

    log::info!("{}", "💰 Cost Breakdown (credits)".bold());
    log::info!(
        "  {:22}{}{:.6}",
        "Transaction Storage:".cyan(),
        "".yellow(),
        storage_cost as f64 / 1_000_000.0
    );
    log::info!(
        "  {:22}{}{:.6}",
        "On-chain Execution:".cyan(),
        "".yellow(),
        execution_cost as f64 / 1_000_000.0
    );
    log::info!(
        "  {:22}{}{:.6}",
        "Priority Fee:".cyan(),
        "".yellow(),
        prio_cr
    );
    log::info!("  {:22}{}{:.6}", "Total Fee:".cyan(), "".yellow(), total_cr);

    log::info!(
        "{}",
        "──────────────────────────────────────────────".dimmed()
    );
    Ok(())
}
